//! MCP-сервер (Model Context Protocol) внутри движка: агент читает модель и
//! расчёты, правит проект обычными обратимыми командами и выгружает файлы.
//!
//! Здесь — протокол JSON-RPC 2.0 (строка запроса → строка ответа) и
//! инструменты. Транспорт stdio и мост к открытому окну — в десктопе;
//! инструменты сохранения и открытия проекта выполняет приложение (`HOST_TOOLS`).

use glam::DVec2;
use op_model::{Command, Element, Finish, OpeningKind, Side, Surface, TileLayout};
use serde_json::{Value, json};

use crate::{
    EngineState,
    edit::{self, WallEnd},
    protocol::{DrawingFormat, DrawingRequest, FromEngine, TableFormat, ToEngine, Tool, View},
};

/// Версия протокола MCP, которую поддерживает сервер.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// Инструменты, которые выполняет приложение (файлы и диалоги), а не движок.
pub const HOST_TOOLS: [&str; 2] = ["save_project", "open_project"];

/// Файл, который приложение должно записать по просьбе агента.
#[derive(Debug, Clone, PartialEq)]
pub struct McpFile {
    /// Путь на диске.
    pub path: String,
    /// Содержимое.
    pub bytes: Vec<u8>,
}

/// Итог одного сообщения: ответ (у уведомлений — нет), события для UI, файлы.
#[derive(Debug, Clone, Default)]
pub struct McpOutcome {
    /// Строка ответа JSON-RPC.
    pub response: Option<String>,
    /// События движка (документ изменился и т. п.) — UI применяет их как обычно.
    pub events: Vec<FromEngine>,
    /// Файлы экспорта для записи.
    pub files: Vec<McpFile>,
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    let mut schema = serde_json::Map::new();
    schema.insert("type".into(), json!("object"));
    schema.insert("properties".into(), properties);
    schema.insert("required".into(), json!(required));
    schema.insert("additionalProperties".into(), json!(false));
    json!({ "name": name, "description": description, "inputSchema": schema })
}

fn point() -> Value {
    json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2,
            "description": "[x, y] в мм; ось Y плана направлена вниз" })
}

/// Описание инструментов для `tools/list`.
#[must_use]
pub fn tools() -> Vec<Value> {
    let kinds = json!({ "type": "string", "enum": ["walls", "openings", "rooms", "furniture", "devices", "plumbing", "pipes", "dimensions", "leaders", "underlays"] });
    vec![
        tool(
            "get_project",
            "Сводка открытого проекта: название, адрес, текущий вид и этаж, этажи, варианты, помещения с площадями, число элементов.",
            json!({}),
            &[],
        ),
        tool(
            "list_elements",
            "Элементы текущего вида (обмер или вариант) и этажа одного типа, как в модели (мм, ось Y вниз).",
            json!({ "type": kinds }),
            &["type"],
        ),
        tool(
            "get_element",
            "Элемент по идентификатору.",
            json!({ "id": { "type": "string" } }),
            &["id"],
        ),
        tool(
            "get_report",
            "Расчёт текущего вида: summary — площади и изменения по вариантам, electrical — группы щита и освещённость, plumbing — трубы и фитинги, finishes — отделка и расход, climate — тёплые полы и воздухообмен, estimate — смета.",
            json!({ "kind": { "type": "string", "enum": ["summary", "electrical", "plumbing", "finishes", "climate", "estimate"] } }),
            &["kind"],
        ),
        tool(
            "list_catalog",
            "Каталоги: furniture — мебель (id для place_furniture), devices — электроточки (kind для place_device), plumbing — сантехника (id для place_plumbing), materials — отделочные материалы, prices — прайс проекта.",
            json!({ "kind": { "type": "string", "enum": ["furniture", "devices", "plumbing", "materials", "prices"] } }),
            &["kind"],
        ),
        tool(
            "add_walls",
            "Цепочка стен по точкам (как инструмент «Стена»: стыки, примыкания и узлы существующих стен учитываются). closed — замкнуть на первую точку.",
            json!({ "points": { "type": "array", "items": point(), "minItems": 2 }, "thickness": { "type": "number", "description": "толщина, мм (по умолчанию 120)" }, "closed": { "type": "boolean" } }),
            &["points"],
        ),
        tool(
            "add_opening",
            "Дверь, окно или арка в стене рядом с точкой (проём встаёт на ось ближайшей стены).",
            json!({ "kind": { "type": "string", "enum": ["door", "window", "arch"] }, "at": point(), "width": { "type": "number" }, "height": { "type": "number" }, "sill": { "type": "number", "description": "низ окна от пола, мм" } }),
            &["kind", "at"],
        ),
        tool(
            "find_rooms",
            "Создать помещения во всех замкнутых контурах стен без помещения.",
            json!({}),
            &[],
        ),
        tool(
            "rename_room",
            "Переименовать помещение.",
            json!({ "id": { "type": "string" }, "name": { "type": "string" } }),
            &["id", "name"],
        ),
        tool(
            "place_furniture",
            "Поставить предмет каталога мебели; у стены он встаёт к ней спинкой (snap=false — точно в точку).",
            json!({ "item": { "type": "string" }, "at": point(), "rotation": { "type": "number", "description": "градусы по часовой стрелке" }, "snap": { "type": "boolean" } }),
            &["item", "at"],
        ),
        tool(
            "place_device",
            "Поставить электроточку или светильник (настенная встаёт на грань ближайшей стены).",
            json!({ "kind": { "type": "string" }, "at": point() }),
            &["kind", "at"],
        ),
        tool(
            "place_plumbing",
            "Поставить вывод, стояк или коллектор сантехники.",
            json!({ "item": { "type": "string" }, "at": point() }),
            &["item", "at"],
        ),
        tool(
            "set_finish",
            "Добавить отделку поверхности помещения (материал — id из list_catalog materials).",
            json!({ "room": { "type": "string" }, "surface": { "type": "string", "enum": ["floor", "ceiling", "walls"] }, "material": { "type": "string" }, "face": { "type": "integer", "description": "одна стена: номер грани (0 — «А»)" }, "bottom": { "type": "number" }, "top": { "type": "number" } }),
            &["room", "surface", "material"],
        ),
        tool(
            "run",
            "Действие движка: auto_circuits — группы щита, fixture_outlets — выводы под приборы, route_pipes — прокладка труб, default_prices — типовой прайс, undo, redo.",
            json!({ "action": { "type": "string", "enum": ["auto_circuits", "fixture_outlets", "route_pipes", "default_prices", "undo", "redo"] } }),
            &["action"],
        ),
        tool(
            "delete",
            "Удалить элементы по идентификаторам (в варианте элементы обмера демонтируются).",
            json!({ "ids": { "type": "array", "items": { "type": "string" } } }),
            &["ids"],
        ),
        tool(
            "set_view",
            "Показать обмер (variant не задан) или вариант перепланировки.",
            json!({ "variant": { "type": "string" } }),
            &[],
        ),
        tool(
            "create_variant",
            "Новый вариант перепланировки (копия другого или пустой); становится текущим.",
            json!({ "name": { "type": "string" }, "copy_of": { "type": "string" } }),
            &["name"],
        ),
        tool(
            "set_level",
            "Перейти на этаж (level не задан — первый).",
            json!({ "level": { "type": "string" } }),
            &[],
        ),
        tool(
            "add_level",
            "Добавить этаж над верхним и перейти на него.",
            json!({ "name": { "type": "string" } }),
            &["name"],
        ),
        tool(
            "apply_command",
            "Произвольная обратимая команда модели (op_model::Command в JSON): put, remove, batch, setFinishes, setPrices и др. Схема — schema/model.schema.json.",
            json!({ "command": { "type": "object" } }),
            &["command"],
        ),
        tool(
            "export",
            "Выгрузить файл: pdf, dxf, svg, xlsx, bundle (PDF+XLSX в ZIP), ifc — чертежи и модель; estimate_xlsx, estimate_csv — смета; prices_csv — прайс. Путь — абсолютный, с расширением формата.",
            json!({ "format": { "type": "string", "enum": ["pdf", "dxf", "svg", "xlsx", "bundle", "ifc", "estimate_xlsx", "estimate_csv", "prices_csv"] }, "path": { "type": "string" }, "variant": { "type": "string" }, "level": { "type": "string" } }),
            &["format", "path"],
        ),
        tool(
            "save_project",
            "Сохранить проект (в его файл; у нового проекта — в path).",
            json!({ "path": { "type": "string" } }),
            &[],
        ),
        tool(
            "open_project",
            "Открыть проект .oplan с диска (несохранённые правки текущего сохраняются или подтверждаются).",
            json!({ "path": { "type": "string" } }),
            &["path"],
        ),
    ]
}

/// Ошибка инструмента: текст для агента.
type ToolResult = Result<Value, String>;

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("нужен параметр «{key}» (строка)"))
}

fn arg_point(args: &Value, key: &str) -> Result<DVec2, String> {
    let p = args
        .get(key)
        .and_then(Value::as_array)
        .filter(|a| a.len() == 2)
        .and_then(|a| Some(DVec2::new(a[0].as_f64()?, a[1].as_f64()?)))
        .ok_or_else(|| format!("нужен параметр «{key}»: [x, y] в мм"))?;
    p.is_finite()
        .then_some(p)
        .ok_or_else(|| "координаты должны быть числами".to_owned())
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Первая ошибка в событиях движка.
fn error_of(events: &[FromEngine]) -> Option<String> {
    events.iter().find_map(|e| match e {
        FromEngine::Error { message } => Some(message.clone()),
        _ => None,
    })
}

impl EngineState {
    /// Обрабатывает строку JSON-RPC от MCP-клиента.
    pub fn mcp_message(&mut self, line: &str) -> McpOutcome {
        let mut out = McpOutcome::default();
        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => {
                out.response = Some(error_response(&Value::Null, -32700, "некорректный JSON"));
                return out;
            }
        };
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "openplanner", "title": "OpenPlanner", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "OpenPlanner — планировщик ремонта квартиры. Координаты плана — миллиметры, ось Y направлена вниз. Правки — обычные команды проекта: их видно в окне и можно отменить (run undo). Начните с get_project.",
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let (result, events, files) = self.mcp_tool(name, &args);
                out.events = events;
                out.files = files;
                Ok(match result {
                    Ok(v) => {
                        json!({ "content": [{ "type": "text", "text": v.to_string() }], "isError": false })
                    }
                    Err(e) => {
                        json!({ "content": [{ "type": "text", "text": e }], "isError": true })
                    }
                })
            }
            m if m.starts_with("notifications/") => return out,
            _ => Err((-32601, format!("метод «{method}» не поддерживается"))),
        };
        // Уведомление (без id) ответа не получает.
        let Some(id) = id else {
            return out;
        };
        out.response = Some(match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }).to_string(),
            Err((code, message)) => error_response(&id, code, &message),
        });
        out
    }

    /// Сообщение движку от имени агента: события и ошибка (если была).
    fn run_message(
        &mut self,
        message: ToEngine,
        events: &mut Vec<FromEngine>,
    ) -> Result<(), String> {
        let got = self.handle(message).events;
        let err = error_of(&got);
        events.extend(got);
        err.map_or(Ok(()), Err)
    }

    fn mcp_tool(
        &mut self,
        name: &str,
        args: &Value,
    ) -> (ToolResult, Vec<FromEngine>, Vec<McpFile>) {
        let mut events = Vec::new();
        let mut files = Vec::new();
        let result = self.mcp_tool_inner(name, args, &mut events, &mut files);
        (result, events, files)
    }

    #[allow(clippy::too_many_lines)] // один разбор на все инструменты
    fn mcp_tool_inner(
        &mut self,
        name: &str,
        args: &Value,
        events: &mut Vec<FromEngine>,
        files: &mut Vec<McpFile>,
    ) -> ToolResult {
        if HOST_TOOLS.contains(&name) {
            return Err("этот инструмент выполняет приложение OpenPlanner".into());
        }
        match name {
            "get_project" => Ok(self.mcp_project()),
            "list_elements" => {
                let doc = &self.projected;
                let list = match arg_str(args, "type")? {
                    "walls" => doc
                        .walls
                        .iter()
                        .map(|w| {
                            let mut v = to_json(w);
                            if let Some((a, b)) = self.derived.wall_axis(w) {
                                v["length"] = json!((a.distance(b) * 10.0).round() / 10.0);
                                v["start"] = json!([a.x, a.y]);
                                v["end"] = json!([b.x, b.y]);
                            }
                            v
                        })
                        .collect(),
                    "openings" => doc.openings.iter().map(to_json).collect(),
                    "rooms" => doc
                        .rooms
                        .iter()
                        .map(|r| {
                            let mut v = to_json(r);
                            if let Some(&i) = self.derived.room_regions.get(&r.id) {
                                v["area"] = json!(op_geom::mm2_to_m2(self.derived.regions[i].area));
                                v["perimeter"] = json!(self.derived.regions[i].perimeter / 1000.0);
                            }
                            v
                        })
                        .collect(),
                    "furniture" => doc.furniture.iter().map(to_json).collect(),
                    "devices" => doc.devices.iter().map(to_json).collect(),
                    "plumbing" => doc.plumbing.iter().map(to_json).collect(),
                    "pipes" => doc.pipes.iter().map(to_json).collect(),
                    "dimensions" => doc.dimensions.iter().map(to_json).collect(),
                    "leaders" => doc.leaders.iter().map(to_json).collect(),
                    "underlays" => doc.underlays.iter().map(to_json).collect(),
                    other => return Err(format!("неизвестный тип «{other}»")),
                };
                Ok(Value::Array(list))
            }
            "get_element" => {
                let id = arg_str(args, "id")?;
                self.history
                    .document()
                    .element(id)
                    .map(|e| to_json(&e))
                    .ok_or_else(|| format!("элемента «{id}» нет"))
            }
            "get_report" => Ok(match arg_str(args, "kind")? {
                "summary" => to_json(&match self.summary_event() {
                    FromEngine::VariantsSummary { views } => views,
                    _ => Vec::new(),
                }),
                "electrical" => to_json(&crate::electric::report(&self.projected, &self.derived)),
                "plumbing" => to_json(&crate::plumbing::report(&self.projected, &self.derived)),
                "finishes" => to_json(&crate::finish::report(&self.projected, &self.derived)),
                "climate" => to_json(&crate::climate::report(&self.projected, &self.derived)),
                "estimate" => to_json(&self.estimate()),
                other => return Err(format!("неизвестный расчёт «{other}»")),
            }),
            "list_catalog" => Ok(match arg_str(args, "kind")? {
                "furniture" => to_json(&crate::furniture::catalog_entries()
                    .into_iter()
                    .map(|e| json!({ "id": e.id, "name": e.name, "category": e.category, "size": [e.width, e.depth, e.height] }))
                    .collect::<Vec<_>>()),
                "devices" => to_json(&crate::electric::catalog_entries()
                    .into_iter()
                    .map(|e| json!({ "kind": e.kind, "name": e.name, "group": e.group, "height": e.height }))
                    .collect::<Vec<_>>()),
                "plumbing" => to_json(&crate::plumbing::catalog_entries()
                    .into_iter()
                    .map(|e| json!({ "id": e.id, "name": e.name, "height": e.height, "diameter": e.diameter }))
                    .collect::<Vec<_>>()),
                "materials" => {
                    let mut all = self.history.document().materials.clone();
                    all.extend(crate::finish::library());
                    to_json(&all)
                }
                "prices" => to_json(&self.history.document().prices),
                other => return Err(format!("неизвестный каталог «{other}»")),
            }),
            "add_walls" => {
                let points: Vec<DVec2> = args
                    .get("points")
                    .and_then(Value::as_array)
                    .ok_or("нужен параметр «points»")?
                    .iter()
                    .map(|p| {
                        p.as_array()
                            .filter(|a| a.len() == 2)
                            .and_then(|a| Some(DVec2::new(a[0].as_f64()?, a[1].as_f64()?)))
                            .filter(|p| p.is_finite())
                            .ok_or_else(|| "точка — [x, y] в мм".to_owned())
                    })
                    .collect::<Result<_, _>>()?;
                if points.len() < 2 {
                    return Err("нужно хотя бы две точки".into());
                }
                let before = self.history.document().walls.len();
                let mut options = self.options;
                if let Some(t) = args.get("thickness").and_then(Value::as_f64) {
                    if !(t.is_finite() && t > 0.0) {
                        return Err("толщина должна быть больше нуля".into());
                    }
                    options.wall_thickness = t;
                }
                self.run_message(ToEngine::SetToolOptions { options }, events)?;
                self.run_message(ToEngine::SetTool { tool: Tool::Wall }, events)?;
                let closed = args.get("closed").and_then(Value::as_bool).unwrap_or(false);
                let chain = points
                    .iter()
                    .copied()
                    .chain(closed.then(|| points[0]))
                    .collect::<Vec<_>>();
                for p in chain {
                    let node = edit::node_at(&self.derived, p);
                    let got = self.wall_click(WallEnd { pos: p, node });
                    if let Some(e) = error_of(&got) {
                        events.extend(got);
                        return Err(e);
                    }
                    events.extend(got);
                }
                self.run_message(ToEngine::Finish, events)?;
                self.run_message(ToEngine::SetTool { tool: Tool::Select }, events)?;
                let added = self.history.document().walls.len().saturating_sub(before);
                Ok(json!({ "wallsAdded": added }))
            }
            "add_opening" => {
                let kind = match arg_str(args, "kind")? {
                    "door" => OpeningKind::Door,
                    "window" => OpeningKind::Window,
                    "arch" => OpeningKind::Arch,
                    other => return Err(format!("неизвестный проём «{other}»")),
                };
                let at = arg_point(args, "at")?;
                let (wall, a, b) = self
                    .projected
                    .walls
                    .iter()
                    .filter_map(|w| self.derived.wall_axis(w).map(|(a, b)| (w, a, b)))
                    .min_by(|x, y| {
                        op_geom::segment::distance(x.1, x.2, at)
                            .total_cmp(&op_geom::segment::distance(y.1, y.2, at))
                    })
                    .ok_or("на плане нет стен")?;
                let (dw, dh, ds) = kind.defaults();
                let width = args.get("width").and_then(Value::as_f64).unwrap_or(dw);
                let len = a.distance(b);
                if len < width {
                    return Err("стена короче проёма".into());
                }
                let offset = (op_geom::segment::project_param(a, b, at) * len)
                    .clamp(width / 2.0, len - width / 2.0);
                let opening = op_model::Opening {
                    id: self.ids.generate(),
                    layer: op_model::LayerKind::Openings.id().into(),
                    variant: None,
                    wall: wall.id.clone(),
                    kind,
                    offset: offset.round(),
                    width,
                    height: args.get("height").and_then(Value::as_f64).unwrap_or(dh),
                    sill: args.get("sill").and_then(Value::as_f64).unwrap_or(ds),
                    hinge_at_end: false,
                    swing: Side::Left,
                };
                let id = opening.id.clone();
                self.mcp_put(vec![Command::Put {
                    element: Element::Opening(opening),
                    index: None,
                }], events)?;
                Ok(json!({ "id": id }))
            }
            "find_rooms" => {
                let before = self.history.document().rooms.len();
                self.run_message(ToEngine::FindRooms, events)?;
                Ok(json!({ "roomsAdded": self.history.document().rooms.len() - before }))
            }
            "rename_room" => {
                let id = arg_str(args, "id")?;
                let mut room = self
                    .history
                    .document()
                    .room(id)
                    .cloned()
                    .ok_or_else(|| format!("помещения «{id}» нет"))?;
                room.name = arg_str(args, "name")?.to_owned();
                self.mcp_put(vec![Command::Put {
                    element: Element::Room(room),
                    index: None,
                }], events)?;
                Ok(json!({ "ok": true }))
            }
            "place_furniture" => {
                let item = arg_str(args, "item")?.to_owned();
                let at = arg_point(args, "at")?;
                self.run_message(ToEngine::FurnitureTool { item }, events)?;
                self.furniture_rotation = args.get("rotation").and_then(Value::as_f64).unwrap_or(0.0);
                let snap = args.get("snap").and_then(Value::as_bool).unwrap_or(true);
                let mut f = self.furniture_candidate(at, !snap).ok_or("предмет не ставится")?;
                f.id = self.ids.generate();
                let id = f.id.clone();
                self.mcp_put(vec![Command::Put { element: Element::Furniture(f), index: None }], events)?;
                self.run_message(ToEngine::SetTool { tool: Tool::Select }, events)?;
                Ok(json!({ "id": id }))
            }
            "place_device" => {
                let kind: op_model::DeviceKind = serde_json::from_value(json!(arg_str(args, "kind")?))
                    .map_err(|_| "неизвестный вид точки: см. list_catalog devices".to_owned())?;
                let at = arg_point(args, "at")?;
                self.run_message(ToEngine::DeviceTool { kind }, events)?;
                let mut d = self.device_candidate(at, false).ok_or("точка не ставится")?;
                d.id = self.ids.generate();
                let id = d.id.clone();
                self.mcp_put(vec![Command::Put { element: Element::Device(d), index: None }], events)?;
                self.run_message(ToEngine::SetTool { tool: Tool::Select }, events)?;
                Ok(json!({ "id": id }))
            }
            "place_plumbing" => {
                let item = arg_str(args, "item")?.to_owned();
                let at = arg_point(args, "at")?;
                self.run_message(ToEngine::PlumbingTool { item }, events)?;
                let mut p = self.plumbing_candidate(at, false).ok_or("точка не ставится")?;
                p.id = self.ids.generate();
                let id = p.id.clone();
                self.mcp_put(vec![Command::Put { element: Element::PlumbingPoint(p), index: None }], events)?;
                self.run_message(ToEngine::SetTool { tool: Tool::Select }, events)?;
                Ok(json!({ "id": id }))
            }
            "set_finish" => {
                let room = arg_str(args, "room")?.to_owned();
                if self.history.document().room(&room).is_none() {
                    return Err(format!("помещения «{room}» нет"));
                }
                let surface = match arg_str(args, "surface")? {
                    "floor" => Surface::Floor,
                    "ceiling" => Surface::Ceiling,
                    "walls" => Surface::Walls,
                    other => return Err(format!("неизвестная поверхность «{other}»")),
                };
                let material = arg_str(args, "material")?.to_owned();
                if crate::finish::material(self.history.document(), &material).is_none() {
                    return Err("материала нет: см. list_catalog materials".into());
                }
                let finish = Finish {
                    id: self.ids.generate(),
                    room,
                    surface,
                    face: args
                        .get("face")
                        .and_then(Value::as_u64)
                        .and_then(|f| u32::try_from(f).ok()),
                    material,
                    bottom: args.get("bottom").and_then(Value::as_f64).unwrap_or(0.0),
                    top: args.get("top").and_then(Value::as_f64),
                    layout: TileLayout::default(),
                };
                let id = finish.id.clone();
                let mut finishes = self.history.document().finishes.clone();
                finishes.push(finish);
                self.mcp_put(vec![Command::SetFinishes { finishes }], events)?;
                Ok(json!({ "id": id }))
            }
            "run" => {
                let message = match arg_str(args, "action")? {
                    "auto_circuits" => ToEngine::AutoCircuits,
                    "fixture_outlets" => ToEngine::FixtureOutlets,
                    "route_pipes" => ToEngine::RoutePipes,
                    "default_prices" => ToEngine::DefaultPrices,
                    "undo" => ToEngine::Undo,
                    "redo" => ToEngine::Redo,
                    other => return Err(format!("неизвестное действие «{other}»")),
                };
                self.run_message(message, events)?;
                Ok(json!({ "ok": true }))
            }
            "delete" => {
                let ids: Vec<String> = args
                    .get("ids")
                    .and_then(Value::as_array)
                    .ok_or("нужен параметр «ids»")?
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect();
                self.run_message(ToEngine::Select { ids }, events)?;
                self.run_message(ToEngine::DeleteSelection, events)?;
                Ok(json!({ "ok": true }))
            }
            "set_view" => {
                let view = match args.get("variant").and_then(Value::as_str) {
                    Some(id) => View::Variant { id: id.to_owned() },
                    None => View::Survey,
                };
                self.run_message(ToEngine::SetView { view }, events)?;
                Ok(json!({ "ok": true }))
            }
            "create_variant" => {
                self.run_message(
                    ToEngine::CreateVariant {
                        name: arg_str(args, "name")?.to_owned(),
                        copy_of: args.get("copy_of").and_then(Value::as_str).map(str::to_owned),
                    },
                    events,
                )?;
                Ok(json!({ "variant": self.variant() }))
            }
            "set_level" => {
                let level = args.get("level").and_then(Value::as_str).map(str::to_owned);
                self.run_message(ToEngine::SetLevel { level }, events)?;
                Ok(json!({ "ok": true }))
            }
            "add_level" => {
                self.run_message(ToEngine::AddLevel { name: arg_str(args, "name")?.to_owned() }, events)?;
                Ok(json!({ "level": self.level }))
            }
            "apply_command" => {
                let command: Command = serde_json::from_value(args.get("command").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("команда не разобрана: {e}"))?;
                self.run_message(ToEngine::Command { command }, events)?;
                Ok(json!({ "ok": true }))
            }
            "export" => {
                let path = arg_str(args, "path")?.to_owned();
                let request = DrawingRequest {
                    variant: args.get("variant").and_then(Value::as_str).map(str::to_owned),
                    level: args.get("level").and_then(Value::as_str).map(str::to_owned),
                    date: args
                        .get("date")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                };
                let format = arg_str(args, "format")?;
                let bytes = match format {
                    "estimate_xlsx" => self.estimate_file(TableFormat::Xlsx)?,
                    "estimate_csv" => self.estimate_file(TableFormat::Csv)?,
                    "prices_csv" => self.prices_file(TableFormat::Csv)?,
                    drawing => {
                        let f: DrawingFormat = serde_json::from_value(json!(drawing))
                            .map_err(|_| format!("неизвестный формат «{drawing}»"))?;
                        self.export_drawings(&request, f).map_err(|e| e.to_string())?
                    }
                };
                let size = bytes.len();
                files.push(McpFile { path: path.clone(), bytes });
                Ok(json!({ "path": path, "bytes": size }))
            }
            other => Err(format!("инструмента «{other}» нет")),
        }
    }

    /// Команды как одно действие истории (с адаптацией к виду и этажу).
    fn mcp_put(
        &mut self,
        commands: Vec<Command>,
        events: &mut Vec<FromEngine>,
    ) -> Result<(), String> {
        let got = self.execute_all(commands);
        let err = error_of(&got);
        events.extend(got);
        err.map_or(Ok(()), Err)
    }

    fn mcp_project(&self) -> Value {
        let doc = self.history.document();
        let rooms: Vec<Value> = self
            .projected
            .rooms
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "name": r.name,
                    "area": self.derived.room_regions.get(&r.id).map(|&i| op_geom::mm2_to_m2(self.derived.regions[i].area)),
                })
            })
            .collect();
        json!({
            "title": doc.meta.title,
            "address": doc.meta.address,
            "ceilingHeight": doc.default_ceiling_height,
            "view": to_json(&self.view),
            "level": self.level,
            "levels": doc.levels.iter().map(|l| json!({ "id": l.id, "name": l.name, "elevation": l.elevation })).collect::<Vec<_>>(),
            "variants": doc.variants.iter().map(|v| json!({ "id": v.id, "name": v.name })).collect::<Vec<_>>(),
            "rooms": rooms,
            "counts": {
                "walls": self.projected.walls.len(),
                "openings": self.projected.openings.len(),
                "furniture": self.projected.furniture.len(),
                "devices": self.projected.devices.len(),
                "plumbing": self.projected.plumbing.len(),
                "pipes": self.projected.pipes.len(),
                "finishes": doc.finishes.len(),
                "prices": doc.prices.len(),
            },
        })
    }
}

fn error_response(id: &Value, code: i32, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> EngineState {
        EngineState::new(DVec2::new(1000.0, 800.0), 1.0)
    }

    #[allow(clippy::needless_pass_by_value)] // в тестах удобнее передавать json!(…) по значению
    fn call(s: &mut EngineState, name: &str, args: Value) -> (Value, McpOutcome) {
        let line = json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": args } }).to_string();
        let out = s.mcp_message(&line);
        let response: Value = serde_json::from_str(out.response.as_deref().unwrap()).unwrap();
        (response["result"].clone(), out)
    }

    fn ok(result: &Value) -> Value {
        assert_eq!(result["isError"], false, "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn handshake_lists_tools_and_ignores_notifications() {
        let mut s = state();
        let out = s.mcp_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#);
        let r: Value = serde_json::from_str(&out.response.unwrap()).unwrap();
        assert_eq!(r["result"]["serverInfo"]["name"], "openplanner");
        assert!(
            s.mcp_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .response
                .is_none()
        );
        let out = s.mcp_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
        let r: Value = serde_json::from_str(&out.response.unwrap()).unwrap();
        let names: Vec<&str> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        for n in ["get_project", "add_walls", "export", "save_project"] {
            assert!(names.contains(&n), "{n}");
        }
        let out = s.mcp_message(r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#);
        assert!(out.response.unwrap().contains("-32601"));
        assert!(s.mcp_message("{oops").response.unwrap().contains("-32700"));
    }

    #[test]
    fn agent_draws_a_flat_and_reads_it_back() {
        let mut s = state();
        let (r, out) = call(
            &mut s,
            "add_walls",
            json!({ "points": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]], "closed": true, "thickness": 200 }),
        );
        assert_eq!(ok(&r)["wallsAdded"], 4);
        assert!(
            out.events
                .iter()
                .any(|e| matches!(e, FromEngine::DocumentChanged { .. }))
        );
        call(
            &mut s,
            "add_walls",
            json!({ "points": [[2000, 0], [2000, 3000]] }),
        );
        let (r, _) = call(&mut s, "find_rooms", json!({}));
        assert_eq!(ok(&r)["roomsAdded"], 2);
        let (r, _) = call(
            &mut s,
            "add_opening",
            json!({ "kind": "door", "at": [2000, 1500] }),
        );
        assert!(ok(&r)["id"].is_string());
        let project = ok(&call(&mut s, "get_project", json!({})).0);
        assert_eq!(project["rooms"].as_array().unwrap().len(), 2);
        assert_eq!(project["counts"]["openings"], 1);
        let rooms = ok(&call(&mut s, "list_elements", json!({ "type": "rooms" })).0);
        let room = rooms[0]["id"].as_str().unwrap().to_owned();
        let (r, _) = call(
            &mut s,
            "place_furniture",
            json!({ "item": "sofa", "at": [1000, 2800] }),
        );
        assert!(ok(&r)["id"].is_string());
        let (r, _) = call(
            &mut s,
            "place_device",
            json!({ "kind": "socket", "at": [1000, 200] }),
        );
        assert!(ok(&r)["id"].is_string());
        let (r, _) = call(
            &mut s,
            "set_finish",
            json!({ "room": room, "surface": "floor", "material": "laminate-oak" }),
        );
        assert!(ok(&r)["id"].is_string());
        call(&mut s, "run", json!({ "action": "default_prices" }));
        let estimate = ok(&call(&mut s, "get_report", json!({ "kind": "estimate" })).0);
        assert!(estimate["total"].as_f64().unwrap() > 0.0);
        // Ошибки — результатом с isError, а не сбоем протокола.
        let (r, _) = call(
            &mut s,
            "place_furniture",
            json!({ "item": "нет", "at": [0, 0] }),
        );
        assert_eq!(r["isError"], true);
        let (r, _) = call(&mut s, "save_project", json!({}));
        assert_eq!(r["isError"], true);
        // Отмена — обычная.
        let walls = s.document().walls.len();
        call(&mut s, "run", json!({ "action": "undo" }));
        assert!(s.document().walls.len() <= walls);
    }

    #[test]
    fn export_hands_files_to_the_host() {
        let mut s = state();
        call(
            &mut s,
            "add_walls",
            json!({ "points": [[0, 0], [4000, 0], [4000, 3000], [0, 3000]], "closed": true }),
        );
        let (r, out) = call(
            &mut s,
            "export",
            json!({ "format": "ifc", "path": "/tmp/кв.ifc" }),
        );
        assert_eq!(ok(&r)["path"], "/tmp/кв.ifc");
        assert!(String::from_utf8_lossy(&out.files[0].bytes).contains("IFCWALL("));
        let (r, out) = call(
            &mut s,
            "export",
            json!({ "format": "pdf", "path": "/tmp/кв.pdf" }),
        );
        assert!(ok(&r)["bytes"].as_u64().unwrap() > 1000);
        assert!(out.files[0].bytes.starts_with(b"%PDF"));
        let (r, _) = call(
            &mut s,
            "export",
            json!({ "format": "doc", "path": "/tmp/x.doc" }),
        );
        assert_eq!(r["isError"], true);
        let (r, _) = call(
            &mut s,
            "apply_command",
            json!({ "command": { "type": "setTitle", "title": "Кв. 12" } }),
        );
        ok(&r);
        assert_eq!(s.document().meta.title, "Кв. 12");
    }
}
