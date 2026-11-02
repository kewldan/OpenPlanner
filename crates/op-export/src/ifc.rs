//! Экспорт модели в IFC 4 (ISO 10303-21, схема `IFC4`, вид Reference View).
//!
//! Записывается здание из простых тел: стены и перекрытия — выдавленные контуры,
//! проёмы — вырезы (`IfcOpeningElement`) с дверями и окнами, помещения — объёмы
//! с площадью, мебель — габаритные бруски. Единицы — миллиметры; ось Y плана
//! (вниз) переворачивается в ось Y модели (вверх).

use std::fmt::Write as _;

use glam::DVec2;

/// Стена: контур в плане, низ и высота, мм.
#[derive(Debug, Clone, PartialEq)]
pub struct IfcWall {
    /// Ключ для `GlobalId` (идентификатор элемента модели).
    pub key: String,
    /// Название.
    pub name: String,
    /// Контур в плане.
    pub outline: Vec<DVec2>,
    /// Высота, мм.
    pub height: f64,
    /// Проёмы в стене.
    pub openings: Vec<IfcOpening>,
}

/// Вид заполнения проёма.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningFill {
    /// Дверь.
    Door,
    /// Окно.
    Window,
    /// Без заполнения (арка).
    None,
}

/// Проём: прямоугольник выреза в плане (на всю толщину стены), низ и высота, мм.
#[derive(Debug, Clone, PartialEq)]
pub struct IfcOpening {
    /// Ключ для `GlobalId`.
    pub key: String,
    /// Контур выреза в плане.
    pub outline: Vec<DVec2>,
    /// Низ от пола, мм.
    pub sill: f64,
    /// Высота, мм.
    pub height: f64,
    /// Ширина, мм.
    pub width: f64,
    /// Заполнение.
    pub fill: OpeningFill,
}

/// Помещение: контур, высота и площадь пола.
#[derive(Debug, Clone, PartialEq)]
pub struct IfcSpace {
    /// Ключ для `GlobalId`.
    pub key: String,
    /// Номер или короткое название.
    pub name: String,
    /// Полное название.
    pub long_name: String,
    /// Контур.
    pub outline: Vec<DVec2>,
    /// Высота, мм.
    pub height: f64,
    /// Площадь пола, м².
    pub area: f64,
}

/// Брусок: мебель или оборудование.
#[derive(Debug, Clone, PartialEq)]
pub struct IfcBox {
    /// Ключ для `GlobalId`.
    pub key: String,
    /// Название.
    pub name: String,
    /// Контур в плане.
    pub outline: Vec<DVec2>,
    /// Низ от пола, мм.
    pub bottom: f64,
    /// Высота, мм.
    pub height: f64,
}

/// Этаж.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IfcStorey {
    /// Ключ для `GlobalId`.
    pub key: String,
    /// Название.
    pub name: String,
    /// Отметка пола, мм.
    pub elevation: f64,
    /// Стены.
    pub walls: Vec<IfcWall>,
    /// Помещения.
    pub spaces: Vec<IfcSpace>,
    /// Мебель.
    pub furniture: Vec<IfcBox>,
}

/// Здание для экспорта.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IfcModel {
    /// Название проекта.
    pub project: String,
    /// Адрес.
    pub address: String,
    /// Дата и время для заголовка (ISO 8601).
    pub timestamp: String,
    /// Этажи.
    pub storeys: Vec<IfcStorey>,
}

/// `GlobalId` IFC: 128 бит в 22 знаках base64 по алфавиту IFC, детерминированно от ключа.
#[must_use]
pub fn global_id(key: &str) -> String {
    const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
    // Два 64-битных FNV-1a с разными начальными значениями.
    let fnv = |seed: u64| {
        key.bytes().fold(seed, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    let value =
        (u128::from(fnv(0xcbf2_9ce4_8422_2325)) << 64) | u128::from(fnv(0x8422_2325_cbf2_9ce4));
    // Первый знак несёт 2 бита, остальные 21 — по 6.
    let mut out = String::with_capacity(22);
    for i in 0..22 {
        let (shift, bits) = if i == 0 { (126, 2) } else { (126 - 6 * i, 6) };
        let mask = (1u128 << bits) - 1;
        #[allow(clippy::cast_possible_truncation)] // индекс < 64
        let index = ((value >> shift) & mask) as usize;
        out.push(char::from(CHARS[index]));
    }
    out
}

/// Строка IFC: апострофы удваиваются, не-ASCII — как `\X2\…\X0\` (UTF-16, шестнадцатерично).
fn text(s: &str) -> String {
    let mut out = String::from("'");
    let mut wide = String::new();
    let flush = |out: &mut String, wide: &mut String| {
        if !wide.is_empty() {
            out.push_str("\\X2\\");
            out.push_str(wide);
            out.push_str("\\X0\\");
            wide.clear();
        }
    };
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_control() {
            flush(&mut out, &mut wide);
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                c => out.push(c),
            }
        } else if !c.is_control() {
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                let _ = write!(wide, "{unit:04X}");
            }
        }
    }
    flush(&mut out, &mut wide);
    out.push('\'');
    out
}

fn real(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0');
    if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_owned()
    }
}

/// Записывающий: номера сущностей и текст раздела DATA.
struct Step {
    data: String,
    next: u32,
}

impl Step {
    fn add(&mut self, entity: &str) -> u32 {
        let id = self.next;
        self.next += 1;
        let _ = writeln!(self.data, "#{id}={entity};");
        id
    }

    fn point2(&mut self, p: DVec2) -> u32 {
        // План — Y вниз, модель — Y вверх.
        self.add(&format!(
            "IFCCARTESIANPOINT(({},{}))",
            real(p.x),
            real(-p.y)
        ))
    }

    fn point3(&mut self, x: f64, y: f64, z: f64) -> u32 {
        self.add(&format!(
            "IFCCARTESIANPOINT(({},{},{}))",
            real(x),
            real(y),
            real(z)
        ))
    }

    fn placement(&mut self, relative: Option<u32>, z: f64) -> u32 {
        let origin = self.point3(0.0, 0.0, z);
        let axis = self.add(&format!("IFCAXIS2PLACEMENT3D(#{origin},$,$)"));
        let rel = relative.map_or_else(|| "$".to_owned(), |r| format!("#{r}"));
        self.add(&format!("IFCLOCALPLACEMENT({rel},#{axis})"))
    }

    /// Тело: контур, выдавленный вверх на высоту.
    fn extrusion(&mut self, context: u32, up: u32, outline: &[DVec2], height: f64) -> u32 {
        let mut points: Vec<u32> = outline.iter().map(|p| self.point2(*p)).collect();
        if let Some(first) = points.first().copied() {
            points.push(first);
        }
        let list = points
            .iter()
            .map(|p| format!("#{p}"))
            .collect::<Vec<_>>()
            .join(",");
        let polyline = self.add(&format!("IFCPOLYLINE(({list}))"));
        let profile = self.add(&format!(
            "IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#{polyline})"
        ));
        let origin = self.point3(0.0, 0.0, 0.0);
        let position = self.add(&format!("IFCAXIS2PLACEMENT3D(#{origin},$,$)"));
        let solid = self.add(&format!(
            "IFCEXTRUDEDAREASOLID(#{profile},#{position},#{up},{})",
            real(height)
        ));
        let rep = self.add(&format!(
            "IFCSHAPEREPRESENTATION(#{context},'Body','SweptSolid',(#{solid}))"
        ));
        self.add(&format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"))
    }
}

fn refs(ids: &[u32]) -> String {
    ids.iter()
        .map(|i| format!("#{i}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Файл IFC 4.
#[must_use]
pub fn write(model: &IfcModel, file_name: &str) -> String {
    let mut s = Step {
        data: String::new(),
        next: 1,
    };
    // Единицы и контекст.
    let length = s.add("IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)");
    let area = s.add("IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.)");
    let volume = s.add("IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.)");
    let angle = s.add("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)");
    let units = s.add(&format!(
        "IFCUNITASSIGNMENT((#{length},#{area},#{volume},#{angle}))"
    ));
    let origin = s.point3(0.0, 0.0, 0.0);
    let world = s.add(&format!("IFCAXIS2PLACEMENT3D(#{origin},$,$)"));
    let north = s.add("IFCDIRECTION((0.,1.))");
    let context = s.add(&format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#{world},#{north})"
    ));
    let body = s.add(&format!(
        "IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{context},$,.MODEL_VIEW.,$)"
    ));
    let up = s.add("IFCDIRECTION((0.,0.,1.))");
    let project = s.add(&format!(
        "IFCPROJECT('{}',$,{},$,$,$,$,(#{context}),#{units})",
        global_id("project"),
        text(&model.project)
    ));
    // Пространственная структура.
    let site_place = s.placement(None, 0.0);
    let site = s.add(&format!(
        "IFCSITE('{}',$,{},$,$,#{site_place},$,$,.ELEMENT.,$,$,$,{},$)",
        global_id("site"),
        text("Участок"),
        text(&model.address)
    ));
    let building_place = s.placement(Some(site_place), 0.0);
    let building = s.add(&format!(
        "IFCBUILDING('{}',$,{},$,$,#{building_place},$,$,.ELEMENT.,$,$,$)",
        global_id("building"),
        text(&model.project)
    ));
    s.add(&format!(
        "IFCRELAGGREGATES('{}',$,$,$,#{project},(#{site}))",
        global_id("rel-project-site")
    ));
    s.add(&format!(
        "IFCRELAGGREGATES('{}',$,$,$,#{site},(#{building}))",
        global_id("rel-site-building")
    ));
    let mut storeys = Vec::new();
    for storey in &model.storeys {
        let place = s.placement(Some(building_place), storey.elevation);
        let id = s.add(&format!(
            "IFCBUILDINGSTOREY('{}',$,{},$,$,#{place},$,$,.ELEMENT.,{})",
            global_id(&format!("storey-{}", storey.key)),
            text(&storey.name),
            real(storey.elevation)
        ));
        storeys.push(id);
        let mut contained = Vec::new();
        for wall in &storey.walls {
            let wall_place = s.placement(Some(place), 0.0);
            let shape = s.extrusion(body, up, &wall.outline, wall.height);
            let w = s.add(&format!(
                "IFCWALL('{}',$,{},$,$,#{wall_place},#{shape},$,.STANDARD.)",
                global_id(&wall.key),
                text(&wall.name)
            ));
            contained.push(w);
            for o in &wall.openings {
                let opening_place = s.placement(Some(wall_place), o.sill);
                let shape = s.extrusion(body, up, &o.outline, o.height);
                let void = s.add(&format!(
                    "IFCOPENINGELEMENT('{}',$,{},$,$,#{opening_place},#{shape},$,.OPENING.)",
                    global_id(&format!("void-{}", o.key)),
                    text("Проём")
                ));
                s.add(&format!(
                    "IFCRELVOIDSELEMENT('{}',$,$,$,#{w},#{void})",
                    global_id(&format!("rel-void-{}", o.key))
                ));
                let fill_place = s.placement(Some(opening_place), 0.0);
                let filling = match o.fill {
                    OpeningFill::Door => Some(s.add(&format!(
                        "IFCDOOR('{}',$,{},$,$,#{fill_place},$,$,{},{},.DOOR.,.SINGLE_SWING_LEFT.,$)",
                        global_id(&o.key),
                        text("Дверь"),
                        real(o.height),
                        real(o.width)
                    ))),
                    OpeningFill::Window => Some(s.add(&format!(
                        "IFCWINDOW('{}',$,{},$,$,#{fill_place},$,$,{},{},.WINDOW.,.DOUBLE_PANEL_VERTICAL.,$)",
                        global_id(&o.key),
                        text("Окно"),
                        real(o.height),
                        real(o.width)
                    ))),
                    OpeningFill::None => None,
                };
                if let Some(f) = filling {
                    s.add(&format!(
                        "IFCRELFILLSELEMENT('{}',$,$,$,#{void},#{f})",
                        global_id(&format!("rel-fill-{}", o.key))
                    ));
                    contained.push(f);
                }
            }
        }
        for f in &storey.furniture {
            let item_place = s.placement(Some(place), f.bottom);
            let shape = s.extrusion(body, up, &f.outline, f.height);
            contained.push(s.add(&format!(
                "IFCFURNITURE('{}',$,{},$,$,#{item_place},#{shape},$,$)",
                global_id(&f.key),
                text(&f.name)
            )));
        }
        if !contained.is_empty() {
            s.add(&format!(
                "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,({}),#{id})",
                global_id(&format!("rel-contained-{}", storey.key)),
                refs(&contained)
            ));
        }
        let mut spaces = Vec::new();
        for space in &storey.spaces {
            let space_place = s.placement(Some(place), 0.0);
            let shape = s.extrusion(body, up, &space.outline, space.height);
            let sp = s.add(&format!(
                "IFCSPACE('{}',$,{},$,$,#{space_place},#{shape},{},.ELEMENT.,.INTERNAL.,$)",
                global_id(&space.key),
                text(&space.name),
                text(&space.long_name)
            ));
            let q = s.add(&format!(
                "IFCQUANTITYAREA('NetFloorArea',$,$,{},$)",
                real(space.area)
            ));
            let set = s.add(&format!(
                "IFCELEMENTQUANTITY('{}',$,'Qto_SpaceBaseQuantities',$,$,(#{q}))",
                global_id(&format!("qto-{}", space.key))
            ));
            s.add(&format!(
                "IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#{sp}),#{set})",
                global_id(&format!("rel-qto-{}", space.key))
            ));
            spaces.push(sp);
        }
        if !spaces.is_empty() {
            s.add(&format!(
                "IFCRELAGGREGATES('{}',$,$,$,#{id},({}))",
                global_id(&format!("rel-spaces-{}", storey.key)),
                refs(&spaces)
            ));
        }
    }
    if !storeys.is_empty() {
        s.add(&format!(
            "IFCRELAGGREGATES('{}',$,$,$,#{building},({}))",
            global_id("rel-building-storeys"),
            refs(&storeys)
        ));
    }
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [ReferenceView]'),'2;1');\nFILE_NAME({},{},(''),(''),'OpenPlanner','OpenPlanner','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{}ENDSEC;\nEND-ISO-10303-21;\n",
        text(file_name),
        text(&model.timestamp),
        s.data
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
        vec![
            DVec2::new(x0, y0),
            DVec2::new(x1, y0),
            DVec2::new(x1, y1),
            DVec2::new(x0, y1),
        ]
    }

    fn model() -> IfcModel {
        IfcModel {
            project: "Кв. 'Комендантский'".into(),
            address: "СПб".into(),
            timestamp: "2026-11-02T12:00:00".into(),
            storeys: vec![IfcStorey {
                key: "l1".into(),
                name: "Этаж 1".into(),
                elevation: 0.0,
                walls: vec![IfcWall {
                    key: "w1".into(),
                    name: "Стена".into(),
                    outline: rect(0.0, 0.0, 4000.0, 200.0),
                    height: 2700.0,
                    openings: vec![IfcOpening {
                        key: "o1".into(),
                        outline: rect(1000.0, -10.0, 1900.0, 210.0),
                        sill: 0.0,
                        height: 2100.0,
                        width: 900.0,
                        fill: OpeningFill::Door,
                    }],
                }],
                spaces: vec![IfcSpace {
                    key: "r1".into(),
                    name: "1".into(),
                    long_name: "Кухня".into(),
                    outline: rect(0.0, 200.0, 4000.0, 3000.0),
                    height: 2700.0,
                    area: 11.2,
                }],
                furniture: vec![IfcBox {
                    key: "f1".into(),
                    name: "Стол".into(),
                    outline: rect(100.0, 300.0, 900.0, 1100.0),
                    bottom: 0.0,
                    height: 750.0,
                }],
            }],
        }
    }

    #[test]
    fn global_ids_are_22_chars_stable_and_distinct() {
        let a = global_id("wall-1");
        assert_eq!(a.len(), 22);
        assert_eq!(a, global_id("wall-1"));
        assert_ne!(a, global_id("wall-2"));
        assert!(matches!(a.as_bytes()[0], b'0'..=b'3'));
    }

    #[test]
    fn strings_are_encoded() {
        assert_eq!(text("a'b"), "'a''b'");
        assert_eq!(text("Ф"), "'\\X2\\0424\\X0\\'");
    }

    #[test]
    fn file_has_structure_and_resolved_references() {
        let ifc = write(&model(), "кв.ifc");
        assert!(ifc.starts_with("ISO-10303-21;") && ifc.contains("FILE_SCHEMA(('IFC4'))"));
        for entity in [
            "IFCPROJECT(",
            "IFCSITE(",
            "IFCBUILDING(",
            "IFCBUILDINGSTOREY(",
            "IFCWALL(",
            "IFCOPENINGELEMENT(",
            "IFCRELVOIDSELEMENT(",
            "IFCDOOR(",
            "IFCRELFILLSELEMENT(",
            "IFCSPACE(",
            "IFCQUANTITYAREA(",
            "IFCFURNITURE(",
            "IFCRELCONTAINEDINSPATIALSTRUCTURE(",
        ] {
            assert!(ifc.contains(entity), "{entity}");
        }
        // Каждая ссылка #N указывает на объявленную сущность.
        let defined: std::collections::HashSet<u32> = ifc
            .lines()
            .filter_map(|l| l.strip_prefix('#')?.split('=').next()?.parse().ok())
            .collect();
        for line in ifc.lines().filter(|l| l.starts_with('#')) {
            let body = line.split_once('=').map_or("", |(_, b)| b);
            let mut rest = body;
            while let Some(i) = rest.find('#') {
                let digits: String = rest[i + 1..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                let n: u32 = digits.parse().unwrap();
                assert!(defined.contains(&n), "#{n} в {line}");
                rest = &rest[i + 1 + digits.len()..];
            }
        }
    }
}
