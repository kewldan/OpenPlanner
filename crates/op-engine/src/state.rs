use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use glam::DVec2;
use op_format::{FormatError, Manifest, journal};
use op_model::{Command, Document, Element, History, HistoryOp, LayerKind, Underlay};
use op_render::{Camera2D, scene::Scene, text::Font};

use crate::{
    derived::Derived,
    draw, geometry,
    ids::IdGen,
    protocol::{EngineEvents, FromEngine, ToEngine, Tool, ToolOptions, ToolStep},
    snap::Snap,
    tools::{Drag, ToolData},
};

/// CSS-пикселей в миллиметре экрана (96 dpi).
pub(crate) const CSS_PX_PER_MM: f64 = 96.0 / 25.4;
/// Знаменатель масштаба, который считается 100 %.
const REFERENCE_SCALE: f64 = 50.0;
/// Название нового проекта.
pub const UNTITLED: &str = "Новый проект";

/// Расшифрованное изображение подложки (RGBA8).
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// Ширина, пиксели.
    pub width: u32,
    /// Высота, пиксели.
    pub height: u32,
    /// Пиксели.
    pub rgba: Arc<Vec<u8>>,
}

/// Изображение, которое надо загрузить в рендерер.
pub type ImageUpload = (String, DecodedImage);

/// Состояние движка без привязки к GPU: документ, история, камера,
/// инструменты, выделение и сцены.
///
/// Вынесено отдельно от WASM-обёртки, чтобы протокол тестировался нативно.
#[derive(Debug)]
pub struct EngineState {
    pub(crate) history: History,
    pub(crate) camera: Camera2D,
    pub(crate) viewport: DVec2,
    pub(crate) device_pixel_ratio: f64,
    pub(crate) ids: IdGen,
    pub(crate) derived: Derived,
    /// Документ с непримёненной правкой (во время перетаскивания).
    pub(crate) preview: Option<(Document, Derived)>,
    pub(crate) tool: Tool,
    pub(crate) options: ToolOptions,
    pub(crate) data: ToolData,
    pub(crate) selection: Vec<String>,
    pub(crate) hover: Option<String>,
    pub(crate) cursor: Option<Snap>,
    pub(crate) drag: Option<Drag>,
    document_dirty: bool,
    pub(crate) overlay_dirty: bool,
    images: HashMap<String, DecodedImage>,
    uploaded: HashSet<String>,
}

impl EngineState {
    /// Новое состояние с пустым документом.
    #[must_use]
    pub fn new(viewport: DVec2, device_pixel_ratio: f64) -> Self {
        let doc = Document::new(UNTITLED);
        Self {
            derived: Derived::new(&doc),
            history: History::new(doc),
            camera: Camera2D::new(),
            viewport,
            device_pixel_ratio,
            ids: IdGen::new(),
            preview: None,
            tool: Tool::Select,
            options: ToolOptions::default(),
            data: ToolData::None,
            selection: Vec::new(),
            hover: None,
            cursor: None,
            drag: None,
            document_dirty: true,
            overlay_dirty: true,
            images: HashMap::new(),
            uploaded: HashSet::new(),
        }
    }

    /// Камера плана.
    #[must_use]
    pub fn camera(&self) -> &Camera2D {
        &self.camera
    }

    /// Размер холста в CSS-пикселях.
    #[must_use]
    pub fn viewport(&self) -> DVec2 {
        self.viewport
    }

    /// Отношение физических пикселей к CSS-пикселям.
    #[must_use]
    pub fn device_pixel_ratio(&self) -> f64 {
        self.device_pixel_ratio
    }

    /// Текущий документ (сохранённое состояние истории).
    #[must_use]
    pub fn document(&self) -> &Document {
        self.history.document()
    }

    /// Выделенные элементы.
    #[must_use]
    pub fn selection(&self) -> &[String] {
        &self.selection
    }

    /// Документ и производные данные для отображения (с учётом превью правки).
    pub(crate) fn shown(&self) -> (&Document, &Derived) {
        match &self.preview {
            Some((doc, derived)) => (doc, derived),
            None => (self.history.document(), &self.derived),
        }
    }

    /// События, описывающие текущее состояние целиком.
    #[must_use]
    pub fn snapshot(&self) -> EngineEvents {
        EngineEvents {
            events: vec![
                self.document_event(),
                self.derived_event(),
                self.history_event(),
                self.view_event(),
                self.selection_event(),
                self.tool_event(),
            ],
        }
    }

    /// Обрабатывает сообщение UI.
    pub fn handle(&mut self, message: ToEngine) -> EngineEvents {
        let events = match message {
            ToEngine::Resize {
                width,
                height,
                device_pixel_ratio,
            } => {
                self.viewport = DVec2::new(width.max(1.0), height.max(1.0));
                self.device_pixel_ratio = device_pixel_ratio.max(0.1);
                vec![]
            }
            ToEngine::Pan { dx, dy } => {
                self.camera.pan(DVec2::new(dx, dy));
                vec![]
            }
            ToEngine::Zoom { x, y, factor } => {
                self.camera.zoom_at(DVec2::new(x, y), self.viewport, factor);
                self.overlay_dirty = true;
                vec![self.view_event()]
            }
            ToEngine::ZoomToFit => {
                self.zoom_to_fit();
                vec![self.view_event()]
            }
            ToEngine::ZoomToScale { denominator } => {
                if denominator.is_finite() && denominator > 0.0 {
                    let factor = CSS_PX_PER_MM / denominator / self.camera.px_per_mm;
                    let center = self.viewport / 2.0;
                    self.camera.zoom_at(center, self.viewport, factor);
                }
                vec![self.view_event()]
            }
            ToEngine::Command { command } => self.apply(HistoryOp::Execute { command }),
            ToEngine::Undo => {
                self.reset_interaction();
                self.apply(HistoryOp::Undo)
            }
            ToEngine::Redo => {
                self.reset_interaction();
                self.apply(HistoryOp::Redo)
            }
            ToEngine::NewDocument => {
                self.reset(Document::new(UNTITLED));
                return self.snapshot();
            }
            ToEngine::MarkSaved => {
                self.history.mark_saved();
                vec![self.history_event()]
            }
            ToEngine::SetTool { tool } => self.set_tool(tool),
            ToEngine::SetToolOptions { options } => {
                if options.wall_thickness.is_finite() && options.wall_thickness > 0.0 {
                    self.options = options;
                    self.overlay_dirty = true;
                    vec![self.tool_event()]
                } else {
                    vec![FromEngine::Error {
                        message: "толщина стены должна быть больше нуля".into(),
                    }]
                }
            }
            ToEngine::Pointer {
                phase,
                x,
                y,
                shift,
                alt,
            } => self.pointer(phase, DVec2::new(x, y), shift, alt),
            ToEngine::Cancel => self.cancel(),
            ToEngine::Finish => self.finish(),
            ToEngine::NumericInput { value, angle } => self.numeric(value, angle),
            ToEngine::Select { ids } => self.select(ids),
            ToEngine::SelectAll => {
                let (lo, hi) = (DVec2::splat(-1e12), DVec2::splat(1e12));
                let ids = crate::hit::in_rect(self.history.document(), &self.derived, lo, hi);
                self.select(ids)
            }
            ToEngine::DeleteSelection => {
                let commands =
                    crate::edit::delete(self.history.document(), &self.derived, &self.selection);
                self.execute_all(commands)
            }
            ToEngine::SetWallLength { id, length } => self.set_wall_length(&id, length),
            ToEngine::FindRooms => self.find_rooms(),
        };
        EngineEvents { events }
    }

    /// Сцены, изменившиеся с прошлого вызова: документ и оверлей.
    pub fn take_scenes(&mut self) -> (Option<Scene>, Option<Scene>) {
        let font = Font::shared();
        let document = self.document_dirty.then(|| {
            let (doc, derived) = self.shown();
            draw::document(doc, derived, font, &|key| self.images.contains_key(key))
        });
        self.document_dirty = false;
        let overlay = self.overlay_dirty.then(|| self.overlay(font));
        self.overlay_dirty = false;
        (document, overlay)
    }

    /// Изображения подложек, которые ещё не загружены в рендерер.
    pub fn take_image_uploads(&mut self) -> Vec<ImageUpload> {
        let mut out = Vec::new();
        for (key, image) in &self.images {
            if self.uploaded.insert(key.clone()) {
                out.push((key.clone(), image.clone()));
            }
        }
        out
    }

    /// Записывает текущий документ в контейнер `.oplan`.
    ///
    /// # Errors
    ///
    /// Ошибка сериализации или упаковки.
    pub fn export_file(&self, generator: &str) -> Result<Vec<u8>, FormatError> {
        let doc = self.history.document();
        let cursor = std::io::Cursor::new(Vec::new());
        let cursor = op_format::write(
            cursor,
            &Manifest::current(generator),
            doc,
            doc.assets.iter().map(|(k, v)| (k.as_str(), v.as_slice())),
        )?;
        Ok(cursor.into_inner())
    }

    /// Открывает проект из контейнера `.oplan`. История начинается заново.
    ///
    /// # Errors
    ///
    /// Ошибка, если файл повреждён или создан более новой версией.
    pub fn import_file(&mut self, bytes: &[u8]) -> Result<EngineEvents, FormatError> {
        let (_manifest, mut document, assets): (Manifest, Document, _) =
            op_format::read(std::io::Cursor::new(bytes))?;
        for (path, data) in assets {
            document.assets.insert(path, Arc::new(data));
        }
        self.reset(document);
        self.zoom_to_fit();
        Ok(self.snapshot())
    }

    /// Воспроизводит журнал автосохранения поверх текущего документа.
    ///
    /// Восстановленные изменения считаются несохранёнными. Строки журнала
    /// повторно не выдаются: журнал продолжает дописываться.
    ///
    /// # Errors
    ///
    /// Ошибка разбора журнала. Операции, которые не применились, пропускаются.
    pub fn replay_journal(&mut self, text: &str) -> Result<EngineEvents, FormatError> {
        let ops: Vec<HistoryOp> = journal::parse(text)?;
        let mut changed = false;
        for op in ops {
            changed |= self.history.apply(op).unwrap_or(false);
        }
        if changed {
            self.history.mark_unsaved();
        }
        self.document_changed();
        self.zoom_to_fit();
        Ok(self.snapshot())
    }

    /// Добавляет изображение-подложку (PNG или JPEG) в центр видимой области.
    ///
    /// # Errors
    ///
    /// Текст ошибки, если файл не читается как изображение.
    pub fn add_underlay(&mut self, bytes: Vec<u8>, name: &str) -> Result<EngineEvents, String> {
        let decoded = decode_image(&bytes)?;
        let id = self.ids.generate();
        let extension = match image::guess_format(&bytes) {
            Ok(image::ImageFormat::Jpeg) => "jpg",
            _ => "png",
        };
        let asset = format!("assets/{id}.{extension}");
        // По умолчанию изображение занимает 80 % ширины видимой области.
        let visible = self.viewport / self.camera.px_per_mm;
        let mm_per_px = (visible.x * 0.8 / f64::from(decoded.width)).max(0.01);
        let size = DVec2::new(f64::from(decoded.width), f64::from(decoded.height)) * mm_per_px;
        let origin = self.camera.center - size / 2.0;
        let underlay = Underlay {
            id: id.clone(),
            layer: LayerKind::Underlays.id().into(),
            asset: asset.clone(),
            name: name.to_owned(),
            x: origin.x,
            y: origin.y,
            width_px: decoded.width,
            height_px: decoded.height,
            mm_per_px,
            opacity: 0.6,
        };
        self.images.insert(asset.clone(), decoded);
        let mut events = self.execute_all(vec![
            Command::asset(asset, bytes),
            Command::Put {
                element: Element::Underlay(underlay),
                index: None,
            },
        ]);
        events.extend(self.select(vec![id]));
        Ok(EngineEvents { events })
    }

    fn reset(&mut self, document: Document) {
        self.history = History::new(document);
        self.camera = Camera2D::new();
        self.selection.clear();
        self.reset_interaction();
        self.tool = Tool::Select;
        self.data = ToolData::None;
        self.images.clear();
        self.uploaded.clear();
        self.document_changed();
    }

    /// Пересчитывает производные данные после изменения документа.
    pub(crate) fn document_changed(&mut self) {
        let doc = self.history.document();
        self.derived = Derived::new(doc);
        self.preview = None;
        let alive: HashSet<&str> = all_ids(doc);
        self.selection.retain(|id| alive.contains(id.as_str()));
        if self
            .hover
            .as_ref()
            .is_some_and(|h| !alive.contains(h.as_str()))
        {
            self.hover = None;
        }
        // Расшифровать изображения новых подложек.
        let missing: Vec<(String, Arc<Vec<u8>>)> = doc
            .underlays
            .iter()
            .filter(|u| !self.images.contains_key(&u.asset))
            .filter_map(|u| {
                doc.assets
                    .get(&u.asset)
                    .map(|d| (u.asset.clone(), d.clone()))
            })
            .collect();
        for (key, data) in missing {
            match decode_image(&data) {
                Ok(image) => {
                    self.images.insert(key, image);
                }
                Err(err) => log::warn!("подложка {key}: {err}"),
            }
        }
        self.document_dirty = true;
        self.overlay_dirty = true;
    }

    /// Показывает превью правки (документ не меняется).
    pub(crate) fn set_preview(&mut self, commands: Vec<Command>) {
        let mut doc = self.history.document().clone();
        if Command::batch(commands).apply(&mut doc).is_ok() {
            let derived = Derived::new(&doc);
            self.preview = Some((doc, derived));
        } else {
            self.preview = None;
        }
        self.document_dirty = true;
        self.overlay_dirty = true;
    }

    pub(crate) fn clear_preview(&mut self) {
        if self.preview.take().is_some() {
            self.document_dirty = true;
        }
        self.overlay_dirty = true;
    }

    /// Выполняет команды одним действием истории.
    pub(crate) fn execute_all(&mut self, commands: Vec<Command>) -> Vec<FromEngine> {
        if commands.is_empty() {
            return vec![];
        }
        self.apply(HistoryOp::Execute {
            command: Command::batch(commands),
        })
    }

    fn apply(&mut self, op: HistoryOp) -> Vec<FromEngine> {
        let line = journal::entry_line(&op);
        match self.history.apply(op) {
            Ok(true) => {
                let before = self.selection.clone();
                self.document_changed();
                let mut events = vec![
                    self.document_event(),
                    self.derived_event(),
                    self.history_event(),
                ];
                if before != self.selection {
                    events.push(self.selection_event());
                }
                match line {
                    Ok(line) => events.push(FromEngine::Journal { line }),
                    Err(err) => events.push(FromEngine::Error {
                        message: err.to_string(),
                    }),
                }
                events
            }
            Ok(false) => vec![],
            Err(err) => {
                self.clear_preview();
                vec![FromEngine::Error {
                    message: err.to_string(),
                }]
            }
        }
    }

    pub(crate) fn select(&mut self, ids: Vec<String>) -> Vec<FromEngine> {
        let alive = all_ids(self.history.document());
        let mut ids: Vec<String> = ids
            .into_iter()
            .filter(|id| alive.contains(id.as_str()))
            .collect();
        ids.dedup();
        if ids == self.selection {
            return vec![];
        }
        self.selection = ids;
        self.overlay_dirty = true;
        vec![self.selection_event()]
    }

    fn set_wall_length(&mut self, id: &str, length: f64) -> Vec<FromEngine> {
        if !length.is_finite() || length < 1.0 {
            return vec![FromEngine::Error {
                message: "длина стены должна быть не меньше 1 мм".into(),
            }];
        }
        let doc = self.history.document();
        let Some(wall) = doc.wall(id) else {
            return vec![];
        };
        let Some((a, b)) = self.derived.wall_axis(wall) else {
            return vec![];
        };
        let dir = (b - a).normalize_or(DVec2::X);
        let to = a + dir * length;
        let commands = crate::edit::move_node(doc, &wall.b, to, None);
        self.execute_all(commands)
    }

    fn find_rooms(&mut self) -> Vec<FromEngine> {
        let doc = self.history.document();
        let used: HashSet<usize> = self.derived.room_regions.values().copied().collect();
        let mut number = doc.rooms.len();
        let mut commands = Vec::new();
        let mut created = Vec::new();
        for (i, region) in self.derived.regions.iter().enumerate() {
            if used.contains(&i) {
                continue;
            }
            let Some(p) = region.label_point() else {
                continue;
            };
            number += 1;
            let id = self.ids.generate();
            created.push(id.clone());
            commands.push(Command::Put {
                element: Element::Room(op_model::Room {
                    id,
                    layer: LayerKind::Rooms.id().into(),
                    name: format!("Помещение {number}"),
                    x: p.x,
                    y: p.y,
                    ceiling_height: None,
                }),
                index: None,
            });
        }
        if commands.is_empty() {
            return vec![FromEngine::Error {
                message: "Новых замкнутых контуров нет".into(),
            }];
        }
        let mut events = self.execute_all(commands);
        events.extend(self.select(created));
        events
    }

    fn zoom_to_fit(&mut self) {
        let (doc, derived) = self.shown();
        let mut points: Vec<DVec2> = derived.nodes.values().copied().collect();
        points.extend(doc.rooms.iter().map(|r| DVec2::new(r.x, r.y)));
        for l in &doc.leaders {
            points.push(DVec2::new(l.x, l.y));
            points.push(DVec2::new(l.text_x, l.text_y));
        }
        for u in &doc.underlays {
            let (a, b) = geometry::underlay_rect(u);
            points.extend([a, b]);
        }
        for d in &doc.dimensions {
            if let Some(g) = geometry::dimension(derived, d) {
                points.extend([g.qa, g.qb]);
            }
        }
        if points.is_empty() {
            self.camera = Camera2D::new();
            self.overlay_dirty = true;
            return;
        }
        let lo = points.iter().fold(DVec2::splat(f64::MAX), |a, p| a.min(*p));
        let hi = points.iter().fold(DVec2::splat(f64::MIN), |a, p| a.max(*p));
        let size = (hi - lo).max(DVec2::splat(1000.0));
        let fit = ((self.viewport - 80.0).max(DVec2::splat(50.0)) / size).min_element();
        self.camera.center = (lo + hi) / 2.0;
        self.camera.px_per_mm = fit.clamp(Camera2D::MIN_PX_PER_MM, Camera2D::MAX_PX_PER_MM);
        self.overlay_dirty = true;
    }

    pub(crate) fn document_event(&self) -> FromEngine {
        FromEngine::DocumentChanged {
            document: self.history.document().clone(),
        }
    }

    pub(crate) fn derived_event(&self) -> FromEngine {
        let (rooms, walls, dimensions, free_regions) = self.derived.infos(self.history.document());
        FromEngine::DerivedChanged {
            rooms,
            walls,
            dimensions,
            free_regions,
        }
    }

    pub(crate) fn history_event(&self) -> FromEngine {
        FromEngine::HistoryChanged {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            dirty: self.history.is_dirty(),
        }
    }

    pub(crate) fn view_event(&self) -> FromEngine {
        let scale_denominator = CSS_PX_PER_MM / self.camera.px_per_mm;
        FromEngine::ViewChanged {
            scale_denominator,
            zoom_percent: REFERENCE_SCALE / scale_denominator * 100.0,
        }
    }

    pub(crate) fn selection_event(&self) -> FromEngine {
        FromEngine::SelectionChanged {
            ids: self.selection.clone(),
        }
    }

    pub(crate) fn tool_event(&self) -> FromEngine {
        FromEngine::ToolChanged {
            tool: self.tool,
            step: self.step(),
            options: self.options,
        }
    }

    /// Текущий шаг инструмента.
    #[must_use]
    pub fn step(&self) -> ToolStep {
        self.data.step(self.tool)
    }
}

fn all_ids(doc: &Document) -> HashSet<&str> {
    doc.walls
        .iter()
        .map(|e| e.id.as_str())
        .chain(doc.openings.iter().map(|e| e.id.as_str()))
        .chain(doc.rooms.iter().map(|e| e.id.as_str()))
        .chain(doc.dimensions.iter().map(|e| e.id.as_str()))
        .chain(doc.leaders.iter().map(|e| e.id.as_str()))
        .chain(doc.underlays.iter().map(|e| e.id.as_str()))
        .collect()
}

/// Расшифровывает PNG или JPEG в RGBA8.
///
/// # Errors
///
/// Текст ошибки для пользователя.
pub fn decode_image(bytes: &[u8]) -> Result<DecodedImage, String> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| format!("не удалось прочитать изображение: {err}"))?;
    let rgba = image.to_rgba8();
    Ok(DecodedImage {
        width: rgba.width(),
        height: rgba.height(),
        rgba: Arc::new(rgba.into_raw()),
    })
}

#[cfg(test)]
impl EngineState {
    /// Глубина стека отмены — для тестов.
    pub(crate) fn history_len_for_tests(&self) -> usize {
        let mut h = self.history.clone();
        let mut n = 0;
        while h.undo().unwrap_or(false) {
            n += 1;
        }
        n
    }
}
