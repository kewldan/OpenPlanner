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
    protocol::{
        DrawingRequest, EngineEvents, FromEngine, ToEngine, Tool, ToolOptions, ToolStep, View,
    },
    sheets::SheetSet,
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
    /// Что показывает холст.
    pub(crate) view: View,
    /// Текущий этаж (`None` — первый).
    pub(crate) level: Option<String>,
    /// Документ в текущем виде (обмер или вариант); инструменты работают с ним.
    pub(crate) projected: Document,
    /// Обмер и его производные — в виде варианта (для показа демонтажа).
    pub(crate) survey: Option<(Document, Derived)>,
    /// Камера до входа в сравнение: при выходе план возвращается туда же.
    camera_before_compare: Option<Camera2D>,
    /// Документ с непримёненной правкой (во время перетаскивания).
    pub(crate) preview: Option<(Document, Derived)>,
    pub(crate) tool: Tool,
    pub(crate) options: ToolOptions,
    pub(crate) data: ToolData,
    pub(crate) selection: Vec<String>,
    pub(crate) hover: Option<String>,
    pub(crate) cursor: Option<Snap>,
    pub(crate) drag: Option<Drag>,
    pub(crate) document_dirty: bool,
    pub(crate) overlay_dirty: bool,
    /// Настройки 3D-вида.
    pub(crate) options_3d: crate::protocol::Options3d,
    /// Камера 3D-вида.
    pub(crate) camera_3d: crate::camera3d::Camera3d,
    /// 3D-сцену надо пересобрать.
    pub(crate) scene_3d_dirty: bool,
    /// Камера 3D уже наведена на модель этого документа.
    pub(crate) camera_3d_fitted: bool,
    /// Вид точки для инструмента «Электроточка».
    pub(crate) device_kind: op_model::DeviceKind,
    /// Позиция каталога для инструмента «Мебель».
    pub(crate) furniture_item: String,
    /// Поворот предмета инструмента «Мебель», градусы.
    pub(crate) furniture_rotation: f64,
    /// Свои модели мебели по путям файлов в проекте.
    pub(crate) models: HashMap<String, Arc<crate::furniture::Model>>,
    /// Позиция каталога для инструмента «Сантехника».
    pub(crate) plumbing_item: String,
    /// Система и диаметр для инструмента «Труба».
    pub(crate) pipe: (op_model::PipeSystem, f64),
    /// Показывать раскладку отделки пола на плане.
    pub(crate) show_finishes: bool,
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
            projected: doc.clone(),
            view: View::Survey,
            level: None,
            survey: None,
            camera_before_compare: None,
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
            options_3d: crate::protocol::Options3d::default(),
            camera_3d: crate::camera3d::Camera3d::default(),
            scene_3d_dirty: true,
            camera_3d_fitted: false,
            device_kind: op_model::DeviceKind::Socket,
            furniture_item: "sofa".into(),
            furniture_rotation: 0.0,
            models: HashMap::new(),
            plumbing_item: "outlet-cold".into(),
            pipe: (op_model::PipeSystem::Cold, 20.0),
            show_finishes: false,
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

    /// Документ в текущем виде (обмер или вариант).
    pub(crate) fn doc(&self) -> &Document {
        &self.projected
    }

    /// Вариант текущего вида.
    pub(crate) fn variant(&self) -> Option<&str> {
        match &self.view {
            View::Variant { id } => Some(id),
            _ => None,
        }
    }

    /// Документ и производные данные для отображения (с учётом превью правки).
    pub(crate) fn shown(&self) -> (&Document, &Derived) {
        match &self.preview {
            Some((doc, derived)) => (doc, derived),
            None => (&self.projected, &self.derived),
        }
    }

    /// Правки с учётом вида: в варианте — новые элементы варианта и демонтаж,
    /// в обмере — каскад по элементам вариантов.
    pub(crate) fn adapt(&self, commands: Vec<Command>) -> Vec<Command> {
        let full = self.history.document();
        let commands = match self.variant() {
            Some(v) => crate::variants::adapt_variant(full, v, self.options.material, commands),
            None => crate::variants::adapt_survey(full, commands),
        };
        // Новые элементы — на текущем этаже.
        match &self.level {
            Some(level) => commands
                .into_iter()
                .map(|c| assign_level(full, level, c))
                .collect(),
            None => commands,
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
                self.summary_event(),
                self.view_mode_event(),
                self.view_3d_event(),
                FromEngine::Catalog {
                    items: crate::furniture::catalog_entries(),
                },
                FromEngine::DeviceCatalog {
                    items: crate::electric::catalog_entries(),
                },
                FromEngine::PlumbingCatalog {
                    items: crate::plumbing::catalog_entries(),
                },
                FromEngine::MaterialLibrary {
                    items: crate::finish::library(),
                },
                self.electrical_event(),
                self.plumbing_event(),
                self.finishes_event(),
                self.climate_event(),
                self.estimate_event(),
                self.level_event(),
            ],
        }
    }

    /// Переключает вид: обмер, вариант или сравнение.
    pub(crate) fn set_view(&mut self, view: View) -> Vec<FromEngine> {
        let full = self.history.document();
        let exists = |v: &Option<String>| v.as_ref().is_none_or(|id| full.variant(id).is_some());
        let valid = match &view {
            View::Survey => true,
            View::Variant { id } => full.variant(id).is_some(),
            View::Compare { left, right } => exists(left) && exists(right),
        };
        if !valid {
            return vec![FromEngine::Error {
                message: "Такого варианта нет".into(),
            }];
        }
        let comparing = matches!(self.view, View::Compare { .. });
        match (comparing, matches!(view, View::Compare { .. })) {
            (false, true) => self.camera_before_compare = Some(self.camera),
            (true, false) => {
                if let Some(camera) = self.camera_before_compare.take() {
                    self.camera = camera;
                }
            }
            _ => {}
        }
        self.view = view;
        self.reset_interaction();
        self.selection.clear();
        self.hover = None;
        self.cursor = None;
        if matches!(self.view, View::Compare { .. }) {
            self.tool = Tool::Select;
        }
        self.document_changed();
        if matches!(self.view, View::Compare { .. }) {
            self.zoom_to_fit();
        }
        vec![
            self.view_mode_event(),
            self.derived_event(),
            self.selection_event(),
            self.tool_event(),
            self.view_event(),
            self.electrical_event(),
            self.plumbing_event(),
            self.finishes_event(),
            self.climate_event(),
            self.estimate_event(),
        ]
    }

    pub(crate) fn view_mode_event(&self) -> FromEngine {
        FromEngine::ViewModeChanged {
            view: self.view.clone(),
        }
    }

    pub(crate) fn summary_event(&self) -> FromEngine {
        let full = self.history.document();
        let mut views = vec![crate::variants::summary(full, None)];
        views.extend(
            full.variants
                .iter()
                .map(|v| crate::variants::summary(full, Some(&v.id))),
        );
        FromEngine::VariantsSummary { views }
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
            ToEngine::SetView { view } => self.set_view(view),
            ToEngine::CreateVariant { name, copy_of } => {
                let (commands, id) = crate::variants::create(
                    self.history.document(),
                    &mut self.ids,
                    &name,
                    copy_of.as_deref(),
                );
                let mut events = self.apply(HistoryOp::Execute {
                    command: Command::batch(commands),
                });
                events.extend(self.set_view(View::Variant { id }));
                events
            }
            ToEngine::DeleteVariant { id } => {
                let commands = crate::variants::delete(self.history.document(), &id);
                if self.variant() == Some(id.as_str()) {
                    self.view = View::Survey;
                }
                let mut events = self.apply(HistoryOp::Execute {
                    command: Command::batch(commands),
                });
                events.push(self.view_mode_event());
                events
            }
            ToEngine::SetDemolished { ids, demolished } => match self.variant() {
                Some(v) => {
                    let commands = crate::variants::set_demolished(
                        self.history.document(),
                        v,
                        &ids,
                        demolished,
                    );
                    self.apply(HistoryOp::Execute {
                        command: Command::batch(commands),
                    })
                }
                None => vec![FromEngine::Error {
                    message: "Демонтаж отмечается в варианте перепланировки".into(),
                }],
            },
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
                let ids = crate::hit::in_rect(self.doc(), &self.derived, lo, hi);
                self.select(ids)
            }
            ToEngine::DeleteSelection => {
                let commands = crate::edit::delete(self.doc(), &self.derived, &self.selection);
                self.execute_all(commands)
            }
            ToEngine::SetWallLength { id, length } => self.set_wall_length(&id, length),
            ToEngine::FindRooms => self.find_rooms(),
            ToEngine::Set3d { options } => self.set_3d(options),
            ToEngine::Camera3d { action } => {
                self.camera_3d.apply(action);
                vec![self.view_3d_event()]
            }
            ToEngine::ResetCamera3d => {
                self.fit_camera_3d();
                vec![self.view_3d_event()]
            }
            ToEngine::SaveView3d { name } => self.save_view_3d(name),
            ToEngine::FurnitureTool { item } => {
                if crate::furniture::catalog_item(&item).is_none() {
                    return EngineEvents {
                        events: vec![FromEngine::Error {
                            message: "Такой позиции нет в каталоге".into(),
                        }],
                    };
                }
                self.furniture_item = item;
                self.set_tool(Tool::Furniture)
            }
            ToEngine::DeviceTool { kind } => {
                self.device_kind = kind;
                self.set_tool(Tool::Device)
            }
            ToEngine::AutoCircuits => {
                let (circuits, devices) = {
                    let ids = &mut self.ids;
                    crate::electric::auto_circuits(&self.projected, &self.derived, || {
                        ids.generate()
                    })
                };
                let mut commands = vec![Command::SetCircuits { circuits }];
                commands.extend(devices.into_iter().map(|d| Command::Put {
                    element: Element::Device(d),
                    index: None,
                }));
                self.execute_all(commands)
            }
            ToEngine::PlumbingTool { item } => {
                if crate::plumbing::item(&item).is_none() {
                    return EngineEvents {
                        events: vec![FromEngine::Error {
                            message: "Такой позиции нет в каталоге".into(),
                        }],
                    };
                }
                self.plumbing_item = item;
                self.set_tool(Tool::Plumbing)
            }
            ToEngine::PipeTool { system, diameter } => {
                if !diameter.is_finite() || diameter <= 0.0 {
                    return EngineEvents {
                        events: vec![FromEngine::Error {
                            message: "диаметр должен быть больше нуля".into(),
                        }],
                    };
                }
                self.pipe = (system, diameter);
                self.set_tool(Tool::Pipe)
            }
            ToEngine::FixtureOutlets => {
                let points = {
                    let ids = &mut self.ids;
                    crate::plumbing::outlets_for_fixtures(&self.projected, &self.derived, || {
                        ids.generate()
                    })
                };
                if points.is_empty() {
                    vec![FromEngine::Error {
                        message: "Нет приборов без выводов: поставьте ванну, раковину, унитаз…"
                            .into(),
                    }]
                } else {
                    self.execute_all(
                        points
                            .into_iter()
                            .map(|p| Command::Put {
                                element: Element::PlumbingPoint(p),
                                index: None,
                            })
                            .collect(),
                    )
                }
            }
            ToEngine::RoutePipes => self.route_pipes(),
            ToEngine::OptimizeLayout { id } => {
                match crate::finish::optimize(&self.projected, &self.derived, &id) {
                    Some(updated) => {
                        let finishes = self
                            .history
                            .document()
                            .finishes
                            .iter()
                            .map(|f| {
                                if f.id == id {
                                    updated.clone()
                                } else {
                                    f.clone()
                                }
                            })
                            .collect();
                        self.execute_all(vec![Command::SetFinishes { finishes }])
                    }
                    None => vec![FromEngine::Error {
                        message: "Старт подбирается для плитки и досок в замкнутом помещении"
                            .into(),
                    }],
                }
            }
            ToEngine::DeleteRender { id } => self.delete_render(&id),
            ToEngine::SetLevel { level } => self.set_level(level),
            ToEngine::AddLevel { name } => self.add_level(name),
            ToEngine::DeleteLevel { id } => self.delete_level(&id),
            ToEngine::DefaultPrices => {
                let mut prices = self.history.document().prices.clone();
                let before = prices.len();
                for item in crate::estimate::template() {
                    if !prices
                        .iter()
                        .any(|p| p.code == item.code || p.id == item.id)
                    {
                        prices.push(item);
                    }
                }
                if prices.len() == before {
                    vec![FromEngine::Error {
                        message: "Все позиции типового прайса уже есть в прайсе проекта".into(),
                    }]
                } else {
                    self.execute_all(vec![Command::SetPrices { prices }])
                }
            }
            ToEngine::ShowFinishes { show } => {
                if self.show_finishes != show {
                    self.show_finishes = show;
                    self.document_dirty = true;
                }
                vec![]
            }
            ToEngine::TransformFurniture { rotate, mirror } => {
                self.transform_furniture(rotate, mirror)
            }
            ToEngine::ApplyView3d { id } => self.apply_view_3d(&id),
        };
        EngineEvents { events }
    }

    /// Сцены, изменившиеся с прошлого вызова: документ и оверлей.
    pub fn take_scenes(&mut self) -> (Option<Scene>, Option<Scene>) {
        let font = Font::shared();
        let document = self.document_dirty.then(|| {
            let images = |key: &str| self.images.contains_key(key);
            if let View::Compare { left, right } = &self.view {
                return self.compare_scene(left.as_deref(), right.as_deref(), font, &images);
            }
            let (doc, derived) = self.shown();
            let tiles = self
                .show_finishes
                .then(|| crate::finish::plan_tiles(doc, derived));
            let mut scene =
                draw::document(doc, derived, font, &images, &self.models, tiles.as_ref());
            if let (Some(v), Some((survey, survey_derived))) = (self.variant(), &self.survey) {
                let ids = op_model::view::demolished(self.history.document(), v);
                draw::demolition(
                    &mut scene,
                    &draw::Demolition {
                        survey,
                        derived: survey_derived,
                        ids: &ids,
                    },
                );
            }
            scene
        });
        self.document_dirty = false;
        let overlay = self.overlay_dirty.then(|| self.overlay(font));
        self.overlay_dirty = false;
        (document, overlay)
    }

    /// Два вида рядом: правый сдвинут вправо на ширину левого и 3 м.
    pub(crate) fn compare_layout(
        &self,
        left: Option<&str>,
        right: Option<&str>,
    ) -> [(Option<String>, DVec2); 2] {
        let full = self.history.document();
        let projected = op_model::view::project_level(full, left, self.level.as_deref());
        let derived = Derived::new(&projected);
        let (lo, hi) = derived.nodes.values().fold(
            (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let width = if lo.x <= hi.x { hi.x - lo.x } else { 5000.0 };
        [
            (left.map(str::to_owned), DVec2::ZERO),
            (right.map(str::to_owned), DVec2::new(width + 3000.0, 0.0)),
        ]
    }

    fn compare_scene(
        &self,
        left: Option<&str>,
        right: Option<&str>,
        font: Option<&Font>,
        images: &dyn Fn(&str) -> bool,
    ) -> Scene {
        let full = self.history.document();
        let mut scene = Scene::new();
        for (variant, offset) in self.compare_layout(left, right) {
            let mut doc =
                op_model::view::project_level(full, variant.as_deref(), self.level.as_deref());
            crate::variants::translate(&mut doc, offset);
            let derived = Derived::new(&doc);
            scene.extend(draw::document(
                &doc,
                &derived,
                font,
                images,
                &self.models,
                None,
            ));
            if let Some(v) = &variant {
                let mut survey = op_model::view::project_level(full, None, self.level.as_deref());
                crate::variants::translate(&mut survey, offset);
                let survey_derived = Derived::new(&survey);
                let ids = op_model::view::demolished(full, v);
                draw::demolition(
                    &mut scene,
                    &draw::Demolition {
                        survey: &survey,
                        derived: &survey_derived,
                        ids: &ids,
                    },
                );
            }
            // Подпись вида над планом.
            let top = derived
                .nodes
                .values()
                .fold(DVec2::splat(f64::MAX), |a, p| a.min(*p));
            if let Some(font) = font
                && top.x < f64::MAX
            {
                let mut style = draw::world_text(320.0, draw::palette::ink());
                style.align = op_render::scene::Align::Left;
                scene.text(
                    font,
                    top - DVec2::new(0.0, 900.0),
                    &crate::variants::view_name(full, variant.as_deref()),
                    style,
                );
            }
        }
        scene
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

    /// Комплект чертежей текущего документа.
    #[must_use]
    pub fn drawings(&self, request: &DrawingRequest) -> SheetSet {
        crate::sheets::build(
            self.history.document(),
            request.variant.as_deref(),
            request.level.as_deref(),
            &request.date,
        )
    }

    /// Файл экспорта чертежей или модели (IFC).
    ///
    /// # Errors
    ///
    /// Ошибка записи файла.
    pub fn export_drawings(
        &self,
        request: &DrawingRequest,
        format: crate::protocol::DrawingFormat,
    ) -> Result<Vec<u8>, op_export::ExportError> {
        let set = self.drawings(request);
        if format == crate::protocol::DrawingFormat::Ifc {
            return Ok(crate::ifc::export(
                self.history.document(),
                request.variant.as_deref(),
                &request.date,
                &set.file_name(format),
            ));
        }
        set.export(format)
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
            level: None,
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

    /// Расшифрованное изображение подложки, которое уже известно (не надо читать заново).
    pub(crate) fn images_insert(&mut self, key: String, image: DecodedImage) {
        self.images.insert(key, image);
    }

    /// Показать весь план (после импорта).
    pub(crate) fn zoom_to_fit_now(&mut self) {
        self.zoom_to_fit();
    }

    /// Расчёт электрики текущего вида.
    pub(crate) fn electrical_event(&self) -> FromEngine {
        FromEngine::Electrical {
            report: crate::electric::report(&self.projected, &self.derived),
        }
    }

    /// Расчёт сантехники текущего вида.
    pub(crate) fn plumbing_event(&self) -> FromEngine {
        FromEngine::Plumbing {
            report: crate::plumbing::report(&self.projected, &self.derived),
        }
    }

    /// Поверхности и расчёт отделки текущего вида.
    pub(crate) fn finishes_event(&self) -> FromEngine {
        FromEngine::Finishes {
            report: crate::finish::report(&self.projected, &self.derived),
        }
    }

    pub(crate) fn level_event(&self) -> FromEngine {
        FromEngine::LevelChanged {
            level: self.level.clone(),
        }
    }

    /// Переходит на этаж: выделение и шаг инструмента сбрасываются.
    pub(crate) fn set_level(&mut self, level: Option<String>) -> Vec<FromEngine> {
        if let Some(l) = &level
            && self.history.document().level(l).is_none()
        {
            return vec![FromEngine::Error {
                message: "Такого этажа нет".into(),
            }];
        }
        self.level = level;
        self.reset_interaction();
        self.selection.clear();
        self.hover = None;
        self.cursor = None;
        self.document_changed();
        vec![
            self.level_event(),
            self.derived_event(),
            self.selection_event(),
            self.tool_event(),
            self.electrical_event(),
            self.plumbing_event(),
            self.finishes_event(),
            self.climate_event(),
        ]
    }

    /// Новый этаж над верхним (отметка — потолок плюс 300 мм перекрытия).
    fn add_level(&mut self, name: String) -> Vec<FromEngine> {
        let doc = self.history.document();
        let top = doc.levels.iter().map(|l| l.elevation).fold(0.0, f64::max);
        let id = self.ids.generate();
        let mut levels = doc.levels.clone();
        levels.push(op_model::Level {
            id: id.clone(),
            name: if name.trim().is_empty() {
                format!("Этаж {}", levels.len() + 2)
            } else {
                name
            },
            elevation: top + doc.default_ceiling_height + 300.0,
        });
        let mut events = self.execute_all(vec![Command::SetLevels { levels }]);
        events.extend(self.set_level(Some(id)));
        events
    }

    /// Удаляет этаж со всеми элементами (обмера и вариантов) одним действием.
    fn delete_level(&mut self, id: &str) -> Vec<FromEngine> {
        let full = self.history.document();
        if full.level(id).is_none() {
            return vec![FromEngine::Error {
                message: "Такого этажа нет".into(),
            }];
        }
        let on = op_model::view::on_level(full, Some(id));
        let derived = Derived::new(&on);
        let ids: Vec<String> = all_ids(&on)
            .into_iter()
            .chain(on.nodes.iter().map(|n| n.id.as_str()))
            .map(str::to_owned)
            .collect();
        let mut commands = crate::edit::delete(&on, &derived, &ids);
        // Узлы, оставшиеся без стен в выделении, удаляются отдельно.
        for n in &on.nodes {
            if !commands
                .iter()
                .any(|c| matches!(c, Command::Remove { id } if id == &n.id))
            {
                commands.push(Command::Remove { id: n.id.clone() });
            }
        }
        commands.push(Command::SetLevels {
            levels: full.levels.iter().filter(|l| l.id != id).cloned().collect(),
        });
        if self.level.as_deref() == Some(id) {
            self.level = None;
        }
        let mut events = self.apply(HistoryOp::Execute {
            command: Command::batch(commands),
        });
        events.push(self.level_event());
        events
    }

    /// Климат текущего вида.
    pub(crate) fn climate_event(&self) -> FromEngine {
        FromEngine::Climate {
            report: crate::climate::report(&self.projected, &self.derived),
        }
    }

    /// Смета текущего вида.
    pub(crate) fn estimate_event(&self) -> FromEngine {
        FromEngine::Estimate {
            report: self.estimate(),
        }
    }

    /// Смета текущего вида по всем этажам (в сравнении — обмер).
    #[must_use]
    pub fn estimate(&self) -> crate::protocol::EstimateReport {
        let full = self.history.document();
        let variant = self.variant();
        let parts = op_model::view::levels(full)
            .into_iter()
            .map(|level| {
                let level = level.as_deref();
                let doc = op_model::view::project_level(full, variant, level);
                let derived = Derived::new(&doc);
                let summary = crate::variants::summary_level(full, variant, level);
                (
                    crate::variants::level_name(full, level),
                    crate::estimate::report(&doc, &derived, &summary),
                )
            })
            .collect();
        crate::estimate::combine(parts)
    }

    /// Файл сметы текущего вида: XLSX (разделы, помещения, закупки) или CSV.
    ///
    /// # Errors
    ///
    /// Ошибка записи книги.
    pub fn estimate_file(&self, format: crate::protocol::TableFormat) -> Result<Vec<u8>, String> {
        let report = self.estimate();
        match format {
            crate::protocol::TableFormat::Xlsx => {
                op_export::xlsx::write(&crate::estimate::worksheets(&report))
                    .map_err(|e| e.to_string())
            }
            crate::protocol::TableFormat::Csv => Ok(crate::estimate::csv(&report).into_bytes()),
        }
    }

    /// Файл прайса проекта: XLSX или CSV.
    ///
    /// # Errors
    ///
    /// Ошибка записи книги.
    pub fn prices_file(&self, format: crate::protocol::TableFormat) -> Result<Vec<u8>, String> {
        let prices = &self.history.document().prices;
        match format {
            crate::protocol::TableFormat::Xlsx => {
                op_export::xlsx::write(&[op_export::xlsx::Worksheet {
                    name: "Прайс".into(),
                    tables: vec![crate::estimate::prices_table(prices)],
                }])
                .map_err(|e| e.to_string())
            }
            crate::protocol::TableFormat::Csv => {
                Ok(crate::estimate::prices_csv(prices).into_bytes())
            }
        }
    }

    /// Импорт прайса из CSV или XLSX: позиции сопоставляются по коду или
    /// наименованию, изменившиеся цены уходят в историю с датой `date`.
    ///
    /// # Errors
    ///
    /// Файл не читается или в нём нет позиций.
    pub fn import_prices(&mut self, bytes: &[u8], date: &str) -> Result<EngineEvents, String> {
        let rows = crate::estimate::read_table(bytes)?;
        let imported = {
            let ids = &mut self.ids;
            crate::estimate::import(&self.history.document().prices, &rows, date, || {
                ids.generate()
            })?
        };
        Ok(EngineEvents {
            events: self.execute_all(vec![Command::SetPrices {
                prices: imported.prices,
            }]),
        })
    }

    /// Прокладывает трубы заново: автоматические трубы вида удаляются, от каждого
    /// вывода без ручной трубы — новая до стояка или коллектора.
    fn route_pipes(&mut self) -> Vec<FromEngine> {
        let (doc, derived) = (&self.projected, &self.derived);
        let mut commands: Vec<Command> = doc
            .pipes
            .iter()
            .filter(|p| p.auto && derived.visible.contains(&p.layer))
            .filter(|p| self.owns(&p.id))
            .map(|p| Command::Remove { id: p.id.clone() })
            .collect();
        let mut kept = doc.clone();
        kept.pipes.retain(|p| !p.auto);
        let pipes = {
            let ids = &mut self.ids;
            crate::plumbing::route_all(&kept, derived, || ids.generate())
        };
        if pipes.is_empty() {
            return vec![FromEngine::Error {
                message: "Нечего прокладывать: нужны выводы и стояк или коллектор их системы"
                    .into(),
            }];
        }
        commands.extend(pipes.into_iter().map(|p| Command::Put {
            element: Element::Pipe(p),
            index: None,
        }));
        self.execute_all(commands)
    }

    /// Добавляет свою модель мебели (GLB) в центр видимой области и выделяет её.
    ///
    /// # Errors
    ///
    /// Файл не читается как GLB.
    pub fn import_furniture(&mut self, bytes: Vec<u8>, name: &str) -> Result<EngineEvents, String> {
        let model = crate::furniture::read_model(&bytes)?;
        let id = self.ids.generate();
        let asset = format!("assets/furniture/{id}.glb");
        let [width, depth, height] = model.size.map(|v| v.round().max(1.0));
        let name = name.trim_end_matches(".glb").trim_end_matches(".GLB");
        let item = op_model::Furniture {
            level: None,
            id: id.clone(),
            layer: LayerKind::Furniture.id().into(),
            variant: None,
            item: crate::furniture::CUSTOM.into(),
            name: if name.is_empty() {
                "Модель".into()
            } else {
                name.to_owned()
            },
            x: self.camera.center.x,
            y: self.camera.center.y,
            rotation: 0.0,
            mirrored: false,
            width,
            depth,
            height,
            elevation: 0.0,
            model: Some(asset.clone()),
        };
        self.models.insert(asset.clone(), Arc::new(model));
        let mut events = self.execute_all(vec![
            Command::asset(asset, bytes),
            Command::Put {
                element: Element::Furniture(item),
                index: None,
            },
        ]);
        events.extend(self.select(vec![id]));
        Ok(EngineEvents { events })
    }

    /// Поворачивает и отражает выделенную мебель или предмет инструмента «Мебель».
    pub(crate) fn transform_furniture(&mut self, rotate: f64, mirror: bool) -> Vec<FromEngine> {
        if !rotate.is_finite() {
            return vec![];
        }
        if self.tool == Tool::Furniture {
            self.furniture_rotation = (self.furniture_rotation + rotate).rem_euclid(360.0);
            self.overlay_dirty = true;
            return vec![];
        }
        let commands: Vec<Command> = self
            .doc()
            .furniture
            .iter()
            .filter(|f| self.selection.contains(&f.id))
            .map(|f| {
                let mut f = f.clone();
                f.rotation = (f.rotation + rotate).rem_euclid(360.0);
                f.mirrored ^= mirror;
                Command::Put {
                    element: Element::Furniture(f),
                    index: None,
                }
            })
            .collect();
        self.execute_all(commands)
    }

    /// Импортирует распознанный скан одной правкой и показывает план целиком.
    pub fn import_scan(
        &mut self,
        request: &crate::protocol::ScanImport,
        source: Option<Vec<u8>>,
    ) -> EngineEvents {
        // Скан — это обмер: импорт всегда в общий вид.
        self.view = View::Survey;
        self.document_changed();
        let commands =
            crate::scan_import::commands(self.history.document(), &mut self.ids, request, source);
        let mut events = self.execute_all(commands);
        if !events.iter().any(|e| matches!(e, FromEngine::Error { .. })) {
            events.extend(self.select(Vec::new()));
            self.zoom_to_fit();
            events.push(self.view_event());
        }
        EngineEvents { events }
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
        self.camera_3d_fitted = false;
        self.models.clear();
        self.document_changed();
        if self.options_3d.enabled {
            self.fit_camera_3d();
        }
    }

    /// Пересчитывает проекцию вида и производные данные после изменения документа.
    pub(crate) fn document_changed(&mut self) {
        let full = self.history.document();
        // Вид на удалённый вариант (например, после отмены его создания) — обмер.
        if let Some(v) = self.variant()
            && full.variant(v).is_none()
        {
            self.view = View::Survey;
        }
        // Этаж удалён (например, отменой его создания) — первый этаж.
        if let Some(l) = &self.level
            && full.level(l).is_none()
        {
            self.level = None;
        }
        let variant = self.variant().map(str::to_owned);
        let level = self.level.clone();
        self.projected = op_model::view::project_level(full, variant.as_deref(), level.as_deref());
        self.derived = Derived::new(&self.projected);
        self.survey = variant.as_ref().map(|_| {
            let survey = op_model::view::project_level(full, None, level.as_deref());
            let derived = Derived::new(&survey);
            (survey, derived)
        });
        self.preview = None;
        let doc = self.history.document();
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
        // Прочитать свои модели мебели.
        let missing: Vec<(String, Arc<Vec<u8>>)> = doc
            .furniture
            .iter()
            .filter_map(|f| f.model.clone())
            .filter(|m| !self.models.contains_key(m))
            .filter_map(|m| doc.assets.get(&m).map(|d| (m, d.clone())))
            .collect();
        for (key, data) in missing {
            match crate::furniture::read_model(&data) {
                Ok(model) => {
                    self.models.insert(key, Arc::new(model));
                }
                Err(err) => log::warn!("модель {key}: {err}"),
            }
        }
        self.document_dirty = true;
        self.overlay_dirty = true;
        self.scene_3d_dirty = true;
    }

    /// Показывает превью правки (документ не меняется).
    pub(crate) fn set_preview(&mut self, commands: Vec<Command>) {
        let commands = self.adapt(commands);
        let mut doc = self.history.document().clone();
        if Command::batch(commands).apply(&mut doc).is_ok() {
            let doc = op_model::view::project_level(&doc, self.variant(), self.level.as_deref());
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
        let commands = self.adapt(commands);
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
                    self.summary_event(),
                    self.view_mode_event(),
                    self.electrical_event(),
                    self.plumbing_event(),
                    self.finishes_event(),
                    self.climate_event(),
                    self.estimate_event(),
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
        let doc = self.doc();
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
        let doc = self.doc();
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
                    level: None,
                    variant: None,
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
        let compare_points: Vec<DVec2> = match &self.view {
            View::Compare { left, right } => self
                .compare_layout(left.as_deref(), right.as_deref())
                .into_iter()
                .flat_map(|(v, offset)| {
                    let doc = op_model::view::project_level(
                        self.history.document(),
                        v.as_deref(),
                        self.level.as_deref(),
                    );
                    Derived::new(&doc)
                        .nodes
                        .values()
                        .map(move |p| *p + offset)
                        .collect::<Vec<_>>()
                })
                .collect(),
            _ => Vec::new(),
        };
        let (doc, derived) = self.shown();
        let mut points: Vec<DVec2> = derived.nodes.values().copied().collect();
        if !compare_points.is_empty() {
            points = compare_points;
            // Место под подписи видов.
            if let Some(top) = points.iter().map(|p| p.y).reduce(f64::min) {
                points.push(DVec2::new(points[0].x, top - 1300.0));
            }
        }
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
            document: Box::new(self.history.document().clone()),
        }
    }

    pub(crate) fn derived_event(&self) -> FromEngine {
        let (rooms, mut walls, dimensions, free_regions) = self.derived.infos(self.doc());
        // Демонтированные стены обмера не входят в вид варианта, но их длина нужна инспектору.
        if let Some((doc, derived)) = &self.survey {
            let (_, survey_walls, _, _) = derived.infos(doc);
            walls.extend(
                survey_walls
                    .into_iter()
                    .filter(|w| self.doc().wall(&w.id).is_none()),
            );
        }
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
        if matches!(self.view, View::Compare { .. }) {
            return ToolStep::Unavailable;
        }
        self.data.step(self.tool)
    }

    /// Можно ли править элемент в текущем виде: в варианте — только его
    /// новые элементы (элементы обмера только демонтируются).
    pub(crate) fn owns(&self, id: &str) -> bool {
        match self.variant() {
            None => true,
            Some(v) => self
                .history
                .document()
                .element(id)
                .is_some_and(|e| e.variant() == Some(v)),
        }
    }
}

/// Помечает этажом новые элементы команды (которых нет в документе и у которых этажа нет).
fn assign_level(doc: &Document, level: &str, command: Command) -> Command {
    match command {
        Command::Put { mut element, index } => {
            if element.level().is_none() && doc.element(element.id()).is_none() {
                element.set_level(Some(level.to_owned()));
            }
            Command::Put { element, index }
        }
        Command::Batch { commands } => Command::Batch {
            commands: commands
                .into_iter()
                .map(|c| assign_level(doc, level, c))
                .collect(),
        },
        other => other,
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
        .chain(doc.furniture.iter().map(|e| e.id.as_str()))
        .chain(doc.devices.iter().map(|e| e.id.as_str()))
        .chain(doc.plumbing.iter().map(|e| e.id.as_str()))
        .chain(doc.pipes.iter().map(|e| e.id.as_str()))
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
