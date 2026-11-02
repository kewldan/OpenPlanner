//! Инструменты холста: выбор и перетаскивание, стены, проёмы, помещения,
//! размеры, выноски, калибровка подложки.

use glam::DVec2;
use op_geom::{segment, shapes};
use op_model::{
    Anchor, Command, Dimension, Element, LayerKind, Leader, Opening, OpeningKind, Room, Side,
};
use op_render::text::Font;

use crate::{
    edit::{self, WallEnd},
    geometry,
    hit::{self, HandleKind},
    protocol::{FromEngine, PointerPhase, SnapKind, Tool, ToolStep},
    snap::{self, Snap, SnapInput},
    state::EngineState,
};

/// Насколько надо сдвинуть указатель, чтобы нажатие стало перетаскиванием, CSS-пиксели.
const DRAG_THRESHOLD_PX: f64 = 3.0;
/// Радиус попадания по ручке, CSS-пиксели.
const HANDLE_RADIUS_PX: f64 = 7.0;
/// Радиус попадания по линии, CSS-пиксели.
const HIT_RADIUS_PX: f64 = 5.0;

/// Данные текущего шага инструмента.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ToolData {
    /// Нет незавершённого шага.
    #[default]
    None,
    /// Цепочка стен.
    Wall {
        /// Предыдущая точка цепочки.
        anchor: WallEnd,
        /// Узел, с которого цепочка началась.
        start: Option<String>,
    },
    /// Размер.
    Dimension {
        /// Первая точка.
        start: (DVec2, Anchor),
        /// Вторая точка (после второго щелчка).
        end: Option<(DVec2, Anchor)>,
    },
    /// Выноска: точка, на которую она указывает.
    Leader {
        /// Точка.
        target: DVec2,
    },
    /// Калибровка подложки.
    Calibrate {
        /// Подложка.
        underlay: String,
        /// Первая точка.
        first: DVec2,
        /// Вторая точка.
        second: Option<DVec2>,
    },
}

impl ToolData {
    /// Шаг для подсказки.
    #[must_use]
    pub fn step(&self, tool: Tool) -> ToolStep {
        match (tool, self) {
            (Tool::Wall, Self::Wall { .. }) => ToolStep::WallNext,
            (Tool::Wall, _) => ToolStep::WallStart,
            (Tool::Door | Tool::Window | Tool::Arch, _) => ToolStep::OpeningPlace,
            (Tool::Room, _) => ToolStep::RoomPlace,
            (Tool::Dimension, Self::Dimension { end: Some(_), .. }) => ToolStep::DimensionOffset,
            (Tool::Dimension, Self::Dimension { .. }) => ToolStep::DimensionEnd,
            (Tool::Dimension, _) => ToolStep::DimensionStart,
            (Tool::Leader, Self::Leader { .. }) => ToolStep::LeaderText,
            (Tool::Leader, _) => ToolStep::LeaderTarget,
            (
                Tool::Calibrate,
                Self::Calibrate {
                    second: Some(_), ..
                },
            ) => ToolStep::CalibrateDistance,
            (Tool::Calibrate, Self::Calibrate { .. }) => ToolStep::CalibrateSecond,
            (Tool::Calibrate, _) => ToolStep::CalibrateFirst,
            (Tool::Select, _) => ToolStep::Idle,
            (Tool::Demolish, _) => ToolStep::DemolishPick,
        }
    }
}

/// Что делает перетаскивание.
#[derive(Debug, Clone, PartialEq)]
pub enum DragKind {
    /// Рамка выделения.
    Marquee {
        /// С Shift — добавить к выделению.
        additive: bool,
    },
    /// Перенос выделения; `grab` — опорная точка, которая привязывается.
    Move {
        /// Опорная точка.
        grab: DVec2,
    },
    /// Ручка.
    Handle(HandleKind),
}

/// Перетаскивание указателем.
#[derive(Debug, Clone, PartialEq)]
pub struct Drag {
    /// Начало, CSS-пиксели.
    pub start_screen: DVec2,
    /// Начало, мм.
    pub start_world: DVec2,
    /// Текущая точка, мм.
    pub current: DVec2,
    /// Превысило порог и стало перетаскиванием.
    pub active: bool,
    /// Вид.
    pub kind: DragKind,
    /// Команды текущего превью.
    pub commands: Vec<Command>,
}

fn put(element: Element) -> Command {
    Command::Put {
        element,
        index: None,
    }
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

impl EngineState {
    fn px(&self) -> f64 {
        self.camera.px_per_mm
    }

    fn to_world(&self, screen: DVec2) -> DVec2 {
        self.camera.screen_to_world(screen, self.viewport)
    }

    pub(crate) fn snap_at(
        &self,
        p: DVec2,
        anchor: Option<DVec2>,
        shift: bool,
        alt: bool,
        exclude: &[String],
    ) -> Snap {
        snap::snap(
            self.doc(),
            &self.derived,
            SnapInput {
                cursor: p,
                px_per_mm: self.px(),
                anchor,
                ortho: shift,
                disabled: alt,
                exclude,
            },
        )
    }

    /// Сбрасывает незавершённые действия указателя.
    pub(crate) fn reset_interaction(&mut self) {
        self.drag = None;
        self.data = ToolData::None;
        self.clear_preview();
    }

    pub(crate) fn set_tool(&mut self, tool: Tool) -> Vec<FromEngine> {
        if matches!(self.view, crate::protocol::View::Compare { .. }) && tool != Tool::Select {
            return vec![FromEngine::Error {
                message: "В сравнении вариантов план не редактируется".into(),
            }];
        }
        if tool == Tool::Demolish && self.variant().is_none() {
            return vec![FromEngine::Error {
                message: "Демонтаж отмечается в режиме «Перепланировка»".into(),
            }];
        }
        self.tool = tool;
        self.reset_interaction();
        self.hover = None;
        self.cursor = None;
        let mut events = vec![self.tool_event()];
        if tool == Tool::Calibrate {
            events.extend(self.start_calibration_hint());
        }
        events
    }

    fn start_calibration_hint(&self) -> Vec<FromEngine> {
        if self.doc().underlays.is_empty() {
            vec![FromEngine::Error {
                message: "Сначала добавьте подложку".into(),
            }]
        } else {
            vec![]
        }
    }

    pub(crate) fn cancel(&mut self) -> Vec<FromEngine> {
        if self.drag.is_some() {
            self.drag = None;
            self.clear_preview();
            return vec![];
        }
        if self.data != ToolData::None {
            self.data = ToolData::None;
            self.overlay_dirty = true;
            return vec![self.tool_event()];
        }
        if self.tool != Tool::Select {
            return self.set_tool(Tool::Select);
        }
        self.select(Vec::new())
    }

    pub(crate) fn finish(&mut self) -> Vec<FromEngine> {
        if matches!(self.data, ToolData::Wall { .. }) {
            self.data = ToolData::None;
            self.overlay_dirty = true;
            return vec![self.tool_event()];
        }
        vec![]
    }

    pub(crate) fn numeric(&mut self, value: f64, angle: Option<f64>) -> Vec<FromEngine> {
        if !value.is_finite() || value <= 0.0 {
            return vec![FromEngine::Error {
                message: "введите положительное число".into(),
            }];
        }
        match self.data.clone() {
            ToolData::Wall { anchor, .. } => {
                let dir = match angle {
                    Some(deg) => {
                        let r = deg.to_radians();
                        DVec2::new(r.cos(), -r.sin())
                    }
                    None => self
                        .cursor
                        .as_ref()
                        .map(|c| c.point - anchor.pos)
                        .filter(|d| d.length() > 1e-6)
                        .map_or(DVec2::X, |d| d.normalize()),
                };
                let pos = anchor.pos + dir * value;
                let node = edit::node_at(&self.derived, pos);
                self.wall_click(WallEnd { pos, node })
            }
            ToolData::Calibrate {
                underlay,
                first,
                second: Some(second),
            } => self.calibrate(&underlay, first, second, value),
            _ => vec![],
        }
    }

    pub(crate) fn pointer(
        &mut self,
        phase: PointerPhase,
        screen: DVec2,
        shift: bool,
        alt: bool,
    ) -> Vec<FromEngine> {
        let p = self.to_world(screen);
        if phase == PointerPhase::Leave {
            self.cursor = None;
            self.hover = None;
            self.overlay_dirty = true;
            return vec![];
        }
        if matches!(self.view, crate::protocol::View::Compare { .. }) {
            return vec![];
        }
        match self.tool {
            Tool::Demolish => self.demolish_pointer(phase, p),
            Tool::Select => self.select_pointer(phase, screen, p, shift, alt),
            Tool::Wall => self.wall_pointer(phase, p, shift, alt),
            Tool::Door | Tool::Window | Tool::Arch => self.opening_pointer(phase, p, alt),
            Tool::Room => self.room_pointer(phase, p),
            Tool::Dimension => self.dimension_pointer(phase, p, shift, alt),
            Tool::Leader => self.leader_pointer(phase, p, shift, alt),
            Tool::Calibrate => self.calibrate_pointer(phase, p, alt),
        }
    }

    fn cursor_event(&self, snap: &Snap, from: Option<DVec2>) -> FromEngine {
        let segment = from.map(|a| snap.point - a).filter(|d| d.length() > 1e-9);
        FromEngine::Cursor {
            x: snap.point.x,
            y: snap.point.y,
            snap: snap.kind,
            length: segment.map(DVec2::length),
            angle: segment.map(|d| {
                // Десятые доли градуса, 0 ≤ угол < 360, без «−0».
                let deg = ((-d.y).atan2(d.x).to_degrees() * 10.0).round() / 10.0;
                deg.rem_euclid(360.0) + 0.0
            }),
        }
    }

    // ---------- Выбор ----------

    fn select_pointer(
        &mut self,
        phase: PointerPhase,
        screen: DVec2,
        p: DVec2,
        shift: bool,
        alt: bool,
    ) -> Vec<FromEngine> {
        let font = Font::shared();
        self.cursor = None;
        let mut events = vec![FromEngine::Cursor {
            x: p.x,
            y: p.y,
            snap: None,
            length: None,
            angle: None,
        }];
        match phase {
            PointerPhase::Down => {
                let doc = self.doc();
                let handle = hit::handles(doc, &self.derived, &self.selection)
                    .into_iter()
                    .filter(|h| self.owns(h.kind.target()))
                    .filter(|h| h.pos.distance(p) * self.px() <= HANDLE_RADIUS_PX)
                    .min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p)));
                let kind = if let Some(h) = handle {
                    DragKind::Handle(h.kind)
                } else if let Some(id) =
                    hit::hit(doc, &self.derived, font, p, HIT_RADIUS_PX / self.px())
                {
                    if shift {
                        let mut ids = self.selection.clone();
                        if let Some(i) = ids.iter().position(|s| s == &id) {
                            ids.remove(i);
                        } else {
                            ids.push(id.clone());
                        }
                        events.extend(self.select(ids));
                    } else if !self.selection.contains(&id) {
                        events.extend(self.select(vec![id.clone()]));
                    }
                    match self.single_opening() {
                        Some(o) if self.selection.len() == 1 => {
                            DragKind::Handle(HandleKind::Opening(o))
                        }
                        _ => DragKind::Move {
                            grab: self.grab_point(&id).unwrap_or(p),
                        },
                    }
                } else {
                    DragKind::Marquee { additive: shift }
                };
                self.drag = Some(Drag {
                    start_screen: screen,
                    start_world: p,
                    current: p,
                    active: false,
                    kind,
                    commands: Vec::new(),
                });
                self.overlay_dirty = true;
            }
            PointerPhase::Move => {
                if let Some(mut drag) = self.drag.take() {
                    drag.current = p;
                    if !drag.active && screen.distance(drag.start_screen) > DRAG_THRESHOLD_PX {
                        drag.active = true;
                    }
                    if drag.active {
                        let commands = self.drag_commands(&drag, shift, alt);
                        if matches!(drag.kind, DragKind::Marquee { .. }) {
                            self.overlay_dirty = true;
                        } else {
                            self.set_preview(commands.clone());
                        }
                        drag.commands = commands;
                    }
                    self.drag = Some(drag);
                } else {
                    let doc = self.doc();
                    let hover = hit::hit(doc, &self.derived, font, p, HIT_RADIUS_PX / self.px())
                        .or_else(|| self.demolished_hit(p));
                    if hover != self.hover {
                        self.hover = hover;
                        self.overlay_dirty = true;
                    }
                }
            }
            PointerPhase::Up => {
                if let Some(drag) = self.drag.take() {
                    match (&drag.kind, drag.active) {
                        (DragKind::Marquee { additive }, true) => {
                            let lo = drag.start_world.min(drag.current);
                            let hi = drag.start_world.max(drag.current);
                            let mut ids = if *additive {
                                self.selection.clone()
                            } else {
                                Vec::new()
                            };
                            for id in hit::in_rect(self.doc(), &self.derived, lo, hi) {
                                if !ids.contains(&id) {
                                    ids.push(id);
                                }
                            }
                            self.overlay_dirty = true;
                            events.extend(self.select(ids));
                        }
                        (DragKind::Marquee { additive: false }, false) => {
                            events.extend(self.select(Vec::new()));
                        }
                        (_, true) => {
                            self.clear_preview();
                            events.extend(self.execute_all(drag.commands));
                        }
                        _ => {}
                    }
                    self.overlay_dirty = true;
                }
            }
            PointerPhase::DoubleClick | PointerPhase::Leave => {}
        }
        events
    }

    fn single_opening(&self) -> Option<String> {
        let doc = self.doc();
        match self.selection.as_slice() {
            [id] if doc.opening(id).is_some() => Some(id.clone()),
            _ => None,
        }
    }

    /// Опорная точка элемента для переноса с привязкой.
    fn grab_point(&self, id: &str) -> Option<DVec2> {
        let doc = self.doc();
        if let Some(w) = doc.wall(id) {
            return self.derived.nodes.get(&w.a).copied();
        }
        if let Some(r) = doc.room(id) {
            return Some(DVec2::new(r.x, r.y));
        }
        if let Some(l) = doc.leader(id) {
            return Some(DVec2::new(l.x, l.y));
        }
        if let Some(u) = doc.underlay(id) {
            return Some(DVec2::new(u.x, u.y));
        }
        if let Some(d) = doc.dimension(id) {
            return geometry::dimension(&self.derived, d).map(|g| g.a);
        }
        None
    }

    fn drag_commands(&self, drag: &Drag, shift: bool, alt: bool) -> Vec<Command> {
        let doc = self.doc();
        let p = drag.current;
        match &drag.kind {
            DragKind::Marquee { .. } => Vec::new(),
            DragKind::Move { grab } => {
                let exclude: Vec<String> = doc
                    .walls
                    .iter()
                    .filter(|w| self.selection.contains(&w.id))
                    .flat_map(|w| [w.a.clone(), w.b.clone()])
                    .collect();
                let target = *grab + (p - drag.start_world);
                let s = self.snap_at(target, Some(*grab), shift, alt, &exclude);
                edit::translate(doc, &self.selection, s.point - *grab)
            }
            DragKind::Handle(HandleKind::Node(node)) => {
                let s = self.snap_at(p, None, shift, alt, std::slice::from_ref(node));
                edit::move_node(doc, node, s.point, s.node.as_deref())
            }
            DragKind::Handle(HandleKind::Opening(id)) => {
                let Some(o) = doc.opening(id) else {
                    return Vec::new();
                };
                let Some((a, b)) = doc.wall(&o.wall).and_then(|w| self.derived.wall_axis(w)) else {
                    return Vec::new();
                };
                let len = a.distance(b);
                let t = segment::project_param(a, b, p);
                let half = o.width / 2.0;
                let offset = round_to(t * len, if alt { 1.0 } else { 10.0 })
                    .clamp(half.min(len / 2.0), (len - half).max(len / 2.0));
                let mut o = o.clone();
                o.offset = offset;
                vec![put(Element::Opening(o))]
            }
            DragKind::Handle(HandleKind::DimensionOffset(id)) => {
                let Some(d) = doc.dimension(id) else {
                    return Vec::new();
                };
                let Some(g) = geometry::dimension(&self.derived, d) else {
                    return Vec::new();
                };
                let mut d = d.clone();
                d.offset = round_to((p - g.a).dot(g.normal), if alt { 1.0 } else { 10.0 });
                vec![put(Element::Dimension(d))]
            }
            DragKind::Handle(HandleKind::DimensionEnd(id, end)) => {
                let Some(d) = doc.dimension(id) else {
                    return Vec::new();
                };
                let s = self.snap_at(p, None, shift, alt, &[]);
                let anchor = match s.node {
                    Some(node) => Anchor::Node { node },
                    None => Anchor::Point {
                        x: s.point.x,
                        y: s.point.y,
                    },
                };
                let mut d = d.clone();
                if *end {
                    d.end = anchor;
                } else {
                    d.start = anchor;
                }
                vec![put(Element::Dimension(d))]
            }
            DragKind::Handle(HandleKind::LeaderPoint(id, text)) => {
                let Some(l) = doc.leader(id) else {
                    return Vec::new();
                };
                let s = self.snap_at(p, None, shift, alt, &[]);
                let mut l = l.clone();
                if *text {
                    l.text_x = s.point.x;
                    l.text_y = s.point.y;
                } else {
                    l.x = s.point.x;
                    l.y = s.point.y;
                }
                vec![put(Element::Leader(l))]
            }
            DragKind::Handle(HandleKind::RoomLabel(id)) => {
                let Some(r) = doc.room(id) else {
                    return Vec::new();
                };
                let mut r = r.clone();
                r.x = p.x;
                r.y = p.y;
                vec![put(Element::Room(r))]
            }
        }
    }

    // ---------- Демонтаж ----------

    /// Демонтированный элемент обмера под точкой (в виде варианта).
    pub(crate) fn demolished_hit(&self, p: DVec2) -> Option<String> {
        let (Some(v), Some((survey, derived))) = (self.variant(), &self.survey) else {
            return None;
        };
        let ids = op_model::view::demolished(self.history.document(), v);
        let mut doc = survey.clone();
        doc.walls.retain(|e| ids.contains(&e.id));
        doc.openings.retain(|e| ids.contains(&e.id));
        doc.rooms.retain(|e| ids.contains(&e.id));
        doc.dimensions.clear();
        doc.leaders.clear();
        doc.underlays.clear();
        hit::hit(&doc, derived, Font::shared(), p, HIT_RADIUS_PX / self.px())
    }

    /// Элемент обмера под точкой, который можно демонтировать.
    pub(crate) fn demolish_candidate(&self, p: DVec2) -> Option<String> {
        let doc = self.doc();
        let id = hit::hit(
            doc,
            &self.derived,
            Font::shared(),
            p,
            HIT_RADIUS_PX / self.px(),
        )?;
        let survey = |v: &Option<String>| v.is_none();
        let ok = doc.wall(&id).is_some_and(|e| survey(&e.variant))
            || doc.opening(&id).is_some_and(|e| survey(&e.variant))
            || doc.room(&id).is_some_and(|e| survey(&e.variant));
        ok.then_some(id)
    }

    fn demolish_pointer(&mut self, phase: PointerPhase, p: DVec2) -> Vec<FromEngine> {
        let mut events = vec![FromEngine::Cursor {
            x: p.x,
            y: p.y,
            snap: None,
            length: None,
            angle: None,
        }];
        let restore = self.demolished_hit(p);
        let target = restore.clone().or_else(|| self.demolish_candidate(p));
        match phase {
            PointerPhase::Move => {
                if target != self.hover {
                    self.hover = target;
                    self.overlay_dirty = true;
                }
            }
            PointerPhase::Down => {
                let Some(v) = self.variant().map(str::to_owned) else {
                    return events;
                };
                let Some(id) = target else {
                    events.push(FromEngine::Error {
                        message: "Демонтируются стены, проёмы и помещения обмера; новые элементы удаляются клавишей Delete".into(),
                    });
                    return events;
                };
                let commands = crate::variants::set_demolished(
                    self.history.document(),
                    &v,
                    &[id],
                    restore.is_none(),
                );
                events.extend(self.execute_all(commands));
                self.hover = None;
                self.overlay_dirty = true;
            }
            _ => {}
        }
        events
    }

    // ---------- Стены ----------

    fn wall_anchor(&self) -> Option<DVec2> {
        match &self.data {
            ToolData::Wall { anchor, .. } => Some(anchor.pos),
            _ => None,
        }
    }

    fn wall_pointer(
        &mut self,
        phase: PointerPhase,
        p: DVec2,
        shift: bool,
        alt: bool,
    ) -> Vec<FromEngine> {
        let anchor = self.wall_anchor();
        let s = self.snap_at(p, anchor, shift, alt, &[]);
        let mut events = vec![self.cursor_event(&s, anchor)];
        self.cursor = Some(s.clone());
        self.overlay_dirty = true;
        match phase {
            PointerPhase::Down => {
                let end = WallEnd {
                    pos: s.point,
                    node: s
                        .node
                        .clone()
                        .or_else(|| edit::node_at(&self.derived, s.point)),
                };
                events.extend(self.wall_click(end));
            }
            PointerPhase::DoubleClick => events.extend(self.finish()),
            _ => {}
        }
        events
    }

    fn wall_click(&mut self, end: WallEnd) -> Vec<FromEngine> {
        let ToolData::Wall { anchor, start } = self.data.clone() else {
            let start = end.node.clone();
            self.data = ToolData::Wall { anchor: end, start };
            return vec![self.tool_event()];
        };
        if anchor.pos.distance(end.pos) < 1.0 {
            return vec![];
        }
        let (commands, node) = edit::new_wall(
            &mut self.ids,
            &anchor,
            &end,
            self.options.wall_thickness,
            self.options.justification,
        );
        // Начальный узел цепочки появляется вместе с первой стеной.
        let first = commands.iter().find_map(|c| match c {
            Command::Put {
                element: Element::Node(n),
                ..
            } if anchor.node.is_none() => Some(n.id.clone()),
            _ => None,
        });
        let start = start.or(first);
        let mut events = self.execute_all(commands);
        let closed = end.node.is_some() && end.node == start;
        self.data = if closed || events.iter().any(|e| matches!(e, FromEngine::Error { .. })) {
            ToolData::None
        } else {
            ToolData::Wall {
                anchor: WallEnd {
                    pos: end.pos,
                    node: Some(node),
                },
                start,
            }
        };
        events.push(self.tool_event());
        events
    }

    // ---------- Проёмы ----------

    fn opening_kind(&self) -> OpeningKind {
        match self.tool {
            Tool::Window => OpeningKind::Window,
            Tool::Arch => OpeningKind::Arch,
            _ => OpeningKind::Door,
        }
    }

    /// Проём, который появится под курсором.
    pub(crate) fn opening_candidate(&self, p: DVec2, alt: bool) -> Option<Opening> {
        let doc = self.doc();
        let radius = HIT_RADIUS_PX / self.px();
        let (wall, a, b) = doc
            .walls
            .iter()
            .filter(|w| self.derived.editable(&w.layer))
            .filter_map(|w| self.derived.wall_axis(w).map(|(a, b)| (w, a, b)))
            .map(|(w, a, b)| (w, a, b, segment::distance(a, b, p)))
            .filter(|(w, .., d)| *d <= w.thickness / 2.0 + radius)
            .min_by(|x, y| x.3.total_cmp(&y.3))
            .map(|(w, a, b, _)| (w, a, b))?;
        let kind = self.opening_kind();
        let (width, height, sill) = kind.defaults();
        let len = a.distance(b);
        if len < width {
            return None;
        }
        let t = segment::project_param(a, b, p);
        let mut offset = round_to(t * len, if alt { 1.0 } else { 10.0 });
        if (offset - len / 2.0).abs() * self.px() <= snap::SNAP_RADIUS_PX {
            offset = len / 2.0;
        }
        let half = width / 2.0;
        let offset = offset.clamp(half, len - half);
        let side = if (p - a).perp_dot(b - a) > 0.0 {
            Side::Left
        } else {
            Side::Right
        };
        Some(Opening {
            variant: None,
            id: String::new(),
            layer: LayerKind::Openings.id().into(),
            wall: wall.id.clone(),
            kind,
            offset,
            width,
            height,
            sill,
            hinge_at_end: false,
            swing: side,
        })
    }

    fn opening_pointer(&mut self, phase: PointerPhase, p: DVec2, alt: bool) -> Vec<FromEngine> {
        self.cursor = Some(Snap {
            point: p,
            kind: None,
            node: None,
            wall: None,
            guides: Vec::new(),
        });
        self.overlay_dirty = true;
        let mut events = vec![FromEngine::Cursor {
            x: p.x,
            y: p.y,
            snap: None,
            length: None,
            angle: None,
        }];
        if phase != PointerPhase::Down {
            return events;
        }
        let Some(mut opening) = self.opening_candidate(p, alt) else {
            events.push(FromEngine::Error {
                message: "Щёлкните по стене, в которой нужен проём".into(),
            });
            return events;
        };
        let doc = self.doc();
        let overlaps = doc.openings.iter().any(|o| {
            o.wall == opening.wall
                && (o.offset - opening.offset).abs() < (o.width + opening.width) / 2.0
        });
        if overlaps {
            events.push(FromEngine::Error {
                message: "Проём пересекается с другим проёмом".into(),
            });
            return events;
        }
        opening.id = self.ids.generate();
        let id = opening.id.clone();
        events.extend(self.execute_all(vec![put(Element::Opening(opening))]));
        events.extend(self.select(vec![id]));
        events
    }

    // ---------- Помещения ----------

    fn room_pointer(&mut self, phase: PointerPhase, p: DVec2) -> Vec<FromEngine> {
        self.cursor = Some(Snap {
            point: p,
            kind: None,
            node: None,
            wall: None,
            guides: Vec::new(),
        });
        self.overlay_dirty = true;
        let mut events = vec![FromEngine::Cursor {
            x: p.x,
            y: p.y,
            snap: None,
            length: None,
            angle: None,
        }];
        if phase != PointerPhase::Down {
            return events;
        }
        let Some(region) = shapes::region_at(&self.derived.regions, p) else {
            events.push(FromEngine::Error {
                message: "Здесь нет замкнутого контура стен".into(),
            });
            return events;
        };
        let existing = self
            .derived
            .room_regions
            .iter()
            .find(|(_, r)| **r == region)
            .map(|(id, _)| id.clone());
        if let Some(id) = existing {
            events.extend(self.select(vec![id]));
            return events;
        }
        let id = self.ids.generate();
        let number = self.doc().rooms.len() + 1;
        events.extend(self.execute_all(vec![put(Element::Room(Room {
            variant: None,
            id: id.clone(),
            layer: LayerKind::Rooms.id().into(),
            name: format!("Помещение {number}"),
            x: p.x,
            y: p.y,
            ceiling_height: None,
        }))]));
        events.extend(self.select(vec![id]));
        events
    }

    // ---------- Размеры ----------

    fn dimension_pointer(
        &mut self,
        phase: PointerPhase,
        p: DVec2,
        shift: bool,
        alt: bool,
    ) -> Vec<FromEngine> {
        let anchor = match &self.data {
            ToolData::Dimension { start, end: None } => Some(start.0),
            _ => None,
        };
        let s = self.snap_at(p, anchor, shift, alt, &[]);
        let mut events = vec![self.cursor_event(&s, anchor)];
        self.cursor = Some(s.clone());
        self.overlay_dirty = true;
        if phase != PointerPhase::Down {
            return events;
        }
        let point = (
            s.point,
            match s.node {
                Some(node) => Anchor::Node { node },
                None => Anchor::Point {
                    x: s.point.x,
                    y: s.point.y,
                },
            },
        );
        match self.data.clone() {
            ToolData::Dimension { start, end: None } => {
                if start.0.distance(point.0) < 1.0 {
                    return events;
                }
                self.data = ToolData::Dimension {
                    start,
                    end: Some(point),
                };
            }
            ToolData::Dimension {
                start,
                end: Some(end),
            } => {
                let id = self.ids.generate();
                let offset = self.dimension_offset(start.0, end.0, p, alt);
                self.data = ToolData::None;
                events.extend(self.execute_all(vec![put(Element::Dimension(Dimension {
                    variant: None,
                    id: id.clone(),
                    layer: LayerKind::Dimensions.id().into(),
                    start: start.1,
                    end: end.1,
                    offset,
                }))]));
            }
            _ => {
                self.data = ToolData::Dimension {
                    start: point,
                    end: None,
                };
            }
        }
        events.push(self.tool_event());
        events
    }

    pub(crate) fn dimension_offset(&self, a: DVec2, b: DVec2, p: DVec2, alt: bool) -> f64 {
        let dir = (b - a).normalize_or(DVec2::X);
        round_to(
            (p - a).dot(op_geom::left_normal(dir)),
            if alt { 1.0 } else { 10.0 },
        )
    }

    // ---------- Выноски ----------

    fn leader_pointer(
        &mut self,
        phase: PointerPhase,
        p: DVec2,
        shift: bool,
        alt: bool,
    ) -> Vec<FromEngine> {
        let anchor = match &self.data {
            ToolData::Leader { target } => Some(*target),
            _ => None,
        };
        let s = self.snap_at(p, anchor, shift, alt, &[]);
        let mut events = vec![self.cursor_event(&s, anchor)];
        self.cursor = Some(s.clone());
        self.overlay_dirty = true;
        if phase != PointerPhase::Down {
            return events;
        }
        match self.data.clone() {
            ToolData::Leader { target } => {
                let id = self.ids.generate();
                self.data = ToolData::None;
                events.extend(self.execute_all(vec![put(Element::Leader(Leader {
                    variant: None,
                    id: id.clone(),
                    layer: LayerKind::Leaders.id().into(),
                    x: target.x,
                    y: target.y,
                    text_x: s.point.x,
                    text_y: s.point.y,
                    text: "Выноска".into(),
                }))]));
                events.extend(self.select(vec![id.clone()]));
                events.push(FromEngine::EditText { id });
            }
            _ => {
                self.data = ToolData::Leader { target: s.point };
            }
        }
        events.push(self.tool_event());
        events
    }

    // ---------- Калибровка подложки ----------

    fn calibration_target(&self, p: DVec2) -> Option<String> {
        let doc = self.doc();
        let selected = doc
            .underlays
            .iter()
            .find(|u| self.selection.contains(&u.id))
            .map(|u| u.id.clone());
        selected.or_else(|| {
            doc.underlays
                .iter()
                .rev()
                .find(|u| {
                    let (lo, hi) = geometry::underlay_rect(u);
                    p.x >= lo.x && p.y >= lo.y && p.x <= hi.x && p.y <= hi.y
                })
                .map(|u| u.id.clone())
        })
    }

    fn calibrate_pointer(&mut self, phase: PointerPhase, p: DVec2, alt: bool) -> Vec<FromEngine> {
        let anchor = match &self.data {
            ToolData::Calibrate {
                first,
                second: None,
                ..
            } => Some(*first),
            _ => None,
        };
        let s = self.snap_at(p, anchor, false, alt, &[]);
        let mut events = vec![self.cursor_event(&s, anchor)];
        self.cursor = Some(s.clone());
        self.overlay_dirty = true;
        if phase != PointerPhase::Down {
            return events;
        }
        match self.data.clone() {
            ToolData::Calibrate {
                underlay,
                first,
                second: None,
            } => {
                if first.distance(p) * self.px() < 5.0 {
                    return events;
                }
                self.data = ToolData::Calibrate {
                    underlay,
                    first,
                    second: Some(p),
                };
            }
            ToolData::Calibrate {
                second: Some(_), ..
            } => return events,
            _ => {
                let Some(underlay) = self.calibration_target(p) else {
                    events.push(FromEngine::Error {
                        message: "Щёлкните по подложке".into(),
                    });
                    return events;
                };
                self.data = ToolData::Calibrate {
                    underlay,
                    first: p,
                    second: None,
                };
            }
        }
        events.push(self.tool_event());
        events
    }

    fn calibrate(&mut self, id: &str, first: DVec2, second: DVec2, real: f64) -> Vec<FromEngine> {
        let Some(u) = self.doc().underlay(id).cloned() else {
            self.data = ToolData::None;
            return vec![self.tool_event()];
        };
        let measured = first.distance(second);
        if measured < 1e-6 {
            return vec![];
        }
        let k = real / measured;
        let origin = first + (DVec2::new(u.x, u.y) - first) * k;
        let mut u = u;
        u.mm_per_px *= k;
        u.x = origin.x;
        u.y = origin.y;
        self.data = ToolData::None;
        self.tool = Tool::Select;
        let mut events = self.execute_all(vec![put(Element::Underlay(u))]);
        events.push(self.tool_event());
        events
    }

    /// Привязка курсора для оверлея — без изменений документа.
    #[must_use]
    pub fn cursor_snap(&self) -> Option<SnapKind> {
        self.cursor.as_ref().and_then(|c| c.kind)
    }
}
