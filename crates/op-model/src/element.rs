//! Элемент документа — значение для обобщённых команд `put` и `remove`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    Dimension, Document, Furniture, Layer, Leader, Node, Opening, Room, Underlay, Variant, Wall,
    command::CommandError,
};

/// Любой элемент документа с идентификатором.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Element {
    /// Слой.
    Layer(Layer),
    /// Узел.
    Node(Node),
    /// Стена.
    Wall(Wall),
    /// Проём.
    Opening(Opening),
    /// Помещение.
    Room(Room),
    /// Размер.
    Dimension(Dimension),
    /// Выноска.
    Leader(Leader),
    /// Подложка.
    Underlay(Underlay),
    /// Мебель.
    Furniture(Furniture),
    /// Вариант перепланировки.
    Variant(Variant),
}

/// Применяет `$body` к списку документа, соответствующему варианту элемента.
macro_rules! each_kind {
    ($doc:expr, $list:ident => $body:expr) => {{
        let doc = $doc;
        let r = {
            let $list = &mut doc.layers;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.nodes;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.walls;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.openings;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.rooms;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.dimensions;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.leaders;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.underlays;
            $body
        };
        if r.is_some() {
            return r;
        }
        let r = {
            let $list = &mut doc.furniture;
            $body
        };
        if r.is_some() {
            return r;
        }
        let $list = &mut doc.variants;
        $body
    }};
}

/// Элемент, хранящийся в списке документа.
pub(crate) trait Stored: Clone + Into<Element> {
    fn id(&self) -> &str;
}

macro_rules! stored {
    ($($ty:ident),*) => {$(
        impl Stored for $ty {
            fn id(&self) -> &str {
                &self.id
            }
        }
        impl From<$ty> for Element {
            fn from(value: $ty) -> Self {
                Self::$ty(value)
            }
        }
    )*};
}
stored!(
    Layer, Node, Wall, Opening, Room, Dimension, Leader, Underlay, Furniture, Variant
);

fn take<T: Stored>(list: &mut Vec<T>, id: &str) -> Option<(Element, usize)> {
    let index = list.iter().position(|e| e.id() == id)?;
    Some((list.remove(index).into(), index))
}

fn find<T: Stored>(list: &mut [T], id: &str) -> Option<Element> {
    list.iter().find(|e| e.id() == id).cloned().map(Into::into)
}

impl Element {
    /// Идентификатор элемента.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Layer(e) => &e.id,
            Self::Node(e) => &e.id,
            Self::Wall(e) => &e.id,
            Self::Opening(e) => &e.id,
            Self::Room(e) => &e.id,
            Self::Dimension(e) => &e.id,
            Self::Leader(e) => &e.id,
            Self::Underlay(e) => &e.id,
            Self::Furniture(e) => &e.id,
            Self::Variant(e) => &e.id,
        }
    }

    /// Вариант, в котором элемент новый (у элементов обмера — нет).
    #[must_use]
    pub fn variant(&self) -> Option<&str> {
        match self {
            Self::Node(e) => e.variant.as_deref(),
            Self::Wall(e) => e.variant.as_deref(),
            Self::Opening(e) => e.variant.as_deref(),
            Self::Room(e) => e.variant.as_deref(),
            Self::Dimension(e) => e.variant.as_deref(),
            Self::Leader(e) => e.variant.as_deref(),
            Self::Furniture(e) => e.variant.as_deref(),
            Self::Layer(_) | Self::Underlay(_) | Self::Variant(_) => None,
        }
    }

    /// Задаёт вариант нового элемента (для слоёв, подложек и вариантов — ничего).
    pub fn set_variant(&mut self, variant: Option<String>) {
        match self {
            Self::Node(e) => e.variant = variant,
            Self::Wall(e) => e.variant = variant,
            Self::Opening(e) => e.variant = variant,
            Self::Room(e) => e.variant = variant,
            Self::Dimension(e) => e.variant = variant,
            Self::Leader(e) => e.variant = variant,
            Self::Furniture(e) => e.variant = variant,
            Self::Layer(_) | Self::Underlay(_) | Self::Variant(_) => {}
        }
    }

    /// Слой элемента (у слоя и узла слоя нет).
    #[must_use]
    pub fn layer(&self) -> Option<&str> {
        match self {
            Self::Layer(_) | Self::Node(_) | Self::Variant(_) => None,
            Self::Wall(e) => Some(&e.layer),
            Self::Opening(e) => Some(&e.layer),
            Self::Room(e) => Some(&e.layer),
            Self::Dimension(e) => Some(&e.layer),
            Self::Leader(e) => Some(&e.layer),
            Self::Underlay(e) => Some(&e.layer),
            Self::Furniture(e) => Some(&e.layer),
        }
    }

    fn same_kind(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// Находит элемент любого типа.
pub(crate) fn get(doc: &mut Document, id: &str) -> Option<Element> {
    each_kind!(doc, list => find(list, id))
}

/// Удаляет элемент любого типа и возвращает его с позицией в списке.
pub(crate) fn remove(doc: &mut Document, id: &str) -> Option<(Element, usize)> {
    each_kind!(doc, list => take(list, id))
}

fn place<T>(list: &mut Vec<T>, value: T, index: Option<usize>, existing: Option<usize>) {
    match (existing, index) {
        (Some(i), _) => list[i] = value,
        (None, Some(i)) => list.insert(i.min(list.len()), value),
        (None, None) => list.push(value),
    }
}

/// Вставляет или заменяет элемент. Возвращает прежнее значение, если было.
pub(crate) fn put(
    doc: &mut Document,
    element: Element,
    index: Option<usize>,
) -> Result<Option<Element>, CommandError> {
    let previous = get(doc, element.id());
    if let Some(previous) = &previous
        && !previous.same_kind(&element)
    {
        return Err(CommandError::Conflict {
            id: element.id().to_owned(),
        });
    }
    validate(doc, &element)?;
    let id = element.id().to_owned();
    macro_rules! put_into {
        ($list:expr, $value:expr) => {{
            let existing = $list.iter().position(|e| e.id == id);
            place(&mut $list, $value, index, existing);
        }};
    }
    match element {
        Element::Layer(e) => put_into!(doc.layers, e),
        Element::Node(e) => put_into!(doc.nodes, e),
        Element::Wall(e) => put_into!(doc.walls, e),
        Element::Opening(e) => put_into!(doc.openings, e),
        Element::Room(e) => put_into!(doc.rooms, e),
        Element::Dimension(e) => put_into!(doc.dimensions, e),
        Element::Leader(e) => put_into!(doc.leaders, e),
        Element::Underlay(e) => put_into!(doc.underlays, e),
        Element::Furniture(e) => put_into!(doc.furniture, e),
        Element::Variant(e) => put_into!(doc.variants, e),
    }
    Ok(previous)
}

fn invalid(field: &'static str, reason: &'static str) -> CommandError {
    CommandError::InvalidValue { field, reason }
}

fn finite(field: &'static str, values: &[f64]) -> Result<(), CommandError> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(invalid(field, "координаты должны быть конечными числами"))
    }
}

fn positive(field: &'static str, value: f64, reason: &'static str) -> Result<(), CommandError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(invalid(field, reason))
    }
}

fn missing(id: &str) -> CommandError {
    CommandError::Missing { id: id.to_owned() }
}

fn validate(doc: &Document, element: &Element) -> Result<(), CommandError> {
    if element.id().is_empty() {
        return Err(invalid("id", "у элемента должен быть идентификатор"));
    }
    if let Some(layer) = element.layer()
        && doc.layer(layer).is_none()
    {
        return Err(missing(layer));
    }
    let scope = element.variant();
    if let Some(v) = scope
        && doc.variant(v).is_none()
    {
        return Err(missing(v));
    }
    // Элемент варианта может опираться на обмер и на свой вариант, но не на чужой.
    let reachable = |other: Option<&str>| other.is_none() || other == scope;
    let foreign = |_: &str| CommandError::InvalidValue {
        field: "variant",
        reason: "элемент ссылается на другой вариант",
    };
    match element {
        Element::Layer(l) => {
            if l.name.trim().is_empty() {
                return Err(invalid("name", "у слоя должно быть название"));
            }
            if let Some(kind) = l.system
                && l.id != kind.id()
            {
                return Err(invalid("system", "системный слой нельзя создать заново"));
            }
        }
        Element::Node(n) => finite("x", &[n.x, n.y])?,
        Element::Wall(w) => {
            for node in [&w.a, &w.b] {
                let Some(n) = doc.node(node) else {
                    return Err(missing(node));
                };
                if !reachable(n.variant.as_deref()) {
                    return Err(foreign(node));
                }
            }
            if w.a == w.b {
                return Err(invalid("b", "стена должна соединять разные узлы"));
            }
            positive("thickness", w.thickness, "толщина должна быть больше нуля")?;
            if let Some(h) = w.height {
                positive("height", h, "высота должна быть больше нуля")?;
            }
        }
        Element::Opening(o) => {
            let Some(wall) = doc.wall(&o.wall) else {
                return Err(missing(&o.wall));
            };
            if !reachable(wall.variant.as_deref()) {
                return Err(foreign(&o.wall));
            }
            finite("offset", &[o.offset])?;
            positive("width", o.width, "ширина должна быть больше нуля")?;
            positive("height", o.height, "высота должна быть больше нуля")?;
            if !o.sill.is_finite() || o.sill < 0.0 {
                return Err(invalid("sill", "высота низа не может быть отрицательной"));
            }
        }
        Element::Room(r) => {
            finite("x", &[r.x, r.y])?;
            if let Some(h) = r.ceiling_height {
                positive("ceilingHeight", h, "высота потолка должна быть больше нуля")?;
            }
        }
        Element::Dimension(d) => {
            finite("offset", &[d.offset])?;
            for anchor in [&d.start, &d.end] {
                match anchor {
                    crate::Anchor::Node { node } if doc.node(node).is_none() => {
                        return Err(missing(node));
                    }
                    crate::Anchor::Node { node }
                        if !reachable(doc.node(node).and_then(|n| n.variant.as_deref())) =>
                    {
                        return Err(foreign(node));
                    }
                    crate::Anchor::Point { x, y } => finite("x", &[*x, *y])?,
                    crate::Anchor::Node { .. } => {}
                }
            }
        }
        Element::Leader(l) => finite("x", &[l.x, l.y, l.text_x, l.text_y])?,
        Element::Underlay(u) => {
            finite("x", &[u.x, u.y])?;
            positive("mmPerPx", u.mm_per_px, "масштаб должен быть больше нуля")?;
            if !(0.0..=1.0).contains(&u.opacity) {
                return Err(invalid("opacity", "непрозрачность — от 0 до 1"));
            }
            if !doc.assets.contains_key(&u.asset) {
                return Err(missing(&u.asset));
            }
        }
        Element::Furniture(f) => {
            finite("x", &[f.x, f.y, f.rotation])?;
            positive("width", f.width, "ширина должна быть больше нуля")?;
            positive("depth", f.depth, "глубина должна быть больше нуля")?;
            positive("height", f.height, "высота должна быть больше нуля")?;
            if !f.elevation.is_finite() || f.elevation < 0.0 {
                return Err(invalid(
                    "elevation",
                    "высота над полом не может быть отрицательной",
                ));
            }
            if let Some(model) = &f.model
                && !doc.assets.contains_key(model)
            {
                return Err(missing(model));
            }
        }
        Element::Variant(v) => {
            if v.name.trim().is_empty() {
                return Err(invalid("name", "у варианта должно быть название"));
            }
        }
    }
    Ok(())
}

/// Проверяет, что на элемент никто не ссылается (перед удалением).
pub(crate) fn ensure_unreferenced(doc: &Document, element: &Element) -> Result<(), CommandError> {
    let id = element.id();
    let referenced = match element {
        Element::Layer(l) => {
            if l.system.is_some() {
                return Err(invalid("id", "системный слой нельзя удалить"));
            }
            doc.walls.iter().any(|e| e.layer == id)
                || doc.openings.iter().any(|e| e.layer == id)
                || doc.rooms.iter().any(|e| e.layer == id)
                || doc.dimensions.iter().any(|e| e.layer == id)
                || doc.leaders.iter().any(|e| e.layer == id)
                || doc.underlays.iter().any(|e| e.layer == id)
                || doc.furniture.iter().any(|e| e.layer == id)
        }
        Element::Node(_) => {
            doc.walls.iter().any(|w| w.a == id || w.b == id)
                || doc.dimensions.iter().any(|d| {
                    [&d.start, &d.end]
                        .iter()
                        .any(|a| matches!(a, crate::Anchor::Node { node } if node == id))
                })
        }
        Element::Wall(_) => doc.openings.iter().any(|o| o.wall == id),
        Element::Variant(_) => {
            let own = |v: &Option<String>| v.as_deref() == Some(id);
            doc.nodes.iter().any(|e| own(&e.variant))
                || doc.walls.iter().any(|e| own(&e.variant))
                || doc.openings.iter().any(|e| own(&e.variant))
                || doc.rooms.iter().any(|e| own(&e.variant))
                || doc.dimensions.iter().any(|e| own(&e.variant))
                || doc.leaders.iter().any(|e| own(&e.variant))
                || doc.furniture.iter().any(|e| own(&e.variant))
        }
        _ => false,
    };
    if referenced {
        Err(CommandError::Referenced { id: id.to_owned() })
    } else {
        Ok(())
    }
}

impl Document {
    /// Элемент любого типа по идентификатору.
    #[must_use]
    pub fn element(&self, id: &str) -> Option<Element> {
        macro_rules! look {
            ($($list:ident => $variant:ident),*) => {$(
                if let Some(e) = self.$list.iter().find(|e| e.id == id) {
                    return Some(Element::$variant(e.clone()));
                }
            )*};
        }
        look!(
            layers => Layer,
            nodes => Node,
            walls => Wall,
            openings => Opening,
            rooms => Room,
            dimensions => Dimension,
            leaders => Leader,
            underlays => Underlay,
            furniture => Furniture,
            variants => Variant
        );
        None
    }
}
