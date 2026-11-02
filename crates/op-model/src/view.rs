//! Вид документа: обмер или вариант перепланировки.
//!
//! Вариант = элементы обмера без демонтированных + новые элементы варианта.
//! Проём демонтированной стены демонтируется вместе с ней.

use std::collections::HashSet;

use crate::{Anchor, Document};

/// Элементы обмера, демонтированные в варианте (с проёмами демонтированных стен).
#[must_use]
pub fn demolished(doc: &Document, variant: &str) -> HashSet<String> {
    let Some(v) = doc.variant(variant) else {
        return HashSet::new();
    };
    let mut ids: HashSet<String> = v.demolished.iter().cloned().collect();
    for o in &doc.openings {
        if o.variant.is_none() && ids.contains(&o.wall) {
            ids.insert(o.id.clone());
        }
    }
    ids
}

/// Документ, каким он выглядит в виде `variant` (`None` — обмер).
///
/// Слои, подложки и двоичные файлы остаются; список вариантов — тоже.
#[must_use]
pub fn project(doc: &Document, variant: Option<&str>) -> Document {
    let removed = variant.map(|v| demolished(doc, v)).unwrap_or_default();
    let keep = |own: &Option<String>, id: &str| match (own.as_deref(), variant) {
        (None, _) => !removed.contains(id),
        (Some(own), Some(v)) => own == v,
        (Some(_), None) => false,
    };
    let mut out = doc.clone();
    out.nodes.retain(|e| keep(&e.variant, &e.id));
    out.walls.retain(|e| keep(&e.variant, &e.id));
    let walls: HashSet<&str> = out.walls.iter().map(|w| w.id.as_str()).collect();
    out.openings
        .retain(|e| keep(&e.variant, &e.id) && walls.contains(e.wall.as_str()));
    out.rooms.retain(|e| keep(&e.variant, &e.id));
    out.leaders.retain(|e| keep(&e.variant, &e.id));
    out.furniture.retain(|e| keep(&e.variant, &e.id));
    let nodes: HashSet<String> = out.nodes.iter().map(|n| n.id.clone()).collect();
    out.dimensions.retain(|e| {
        keep(&e.variant, &e.id)
            && [&e.start, &e.end]
                .iter()
                .all(|a| !matches!(a, Anchor::Node { node } if !nodes.contains(node)))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Command, Element, Justification, LayerKind, Node, Opening, OpeningKind, Side, Variant, Wall,
    };

    fn put(e: Element) -> Command {
        Command::Put {
            element: e,
            index: None,
        }
    }

    fn node(id: &str, x: f64, variant: Option<&str>) -> Command {
        put(Element::Node(Node {
            id: id.into(),
            variant: variant.map(Into::into),
            x,
            y: 0.0,
        }))
    }

    fn wall(id: &str, a: &str, b: &str, variant: Option<&str>) -> Command {
        put(Element::Wall(Wall {
            id: id.into(),
            layer: LayerKind::Walls.id().into(),
            variant: variant.map(Into::into),
            a: a.into(),
            b: b.into(),
            thickness: 100.0,
            justification: Justification::Center,
            height: None,
            material: None,
        }))
    }

    fn doc() -> Document {
        let mut doc = Document::new("т");
        Command::Batch {
            commands: vec![
                put(Element::Variant(Variant {
                    id: "A".into(),
                    name: "Вариант А".into(),
                    demolished: vec!["w1".into()],
                })),
                put(Element::Variant(Variant {
                    id: "B".into(),
                    name: "Вариант Б".into(),
                    demolished: Vec::new(),
                })),
                node("n1", 0.0, None),
                node("n2", 1000.0, None),
                wall("w1", "n1", "n2", None),
                put(Element::Opening(Opening {
                    id: "o1".into(),
                    layer: LayerKind::Openings.id().into(),
                    variant: None,
                    wall: "w1".into(),
                    kind: OpeningKind::Door,
                    offset: 500.0,
                    width: 800.0,
                    height: 2100.0,
                    sill: 0.0,
                    hinge_at_end: false,
                    swing: Side::Left,
                })),
                node("n3", 2000.0, Some("A")),
                wall("w2", "n2", "n3", Some("A")),
            ],
        }
        .apply(&mut doc)
        .unwrap();
        doc
    }

    #[test]
    fn survey_hides_variant_elements() {
        let d = project(&doc(), None);
        assert_eq!(d.walls.len(), 1);
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.openings.len(), 1);
    }

    #[test]
    fn variant_demolishes_and_adds() {
        let d = project(&doc(), Some("A"));
        let ids: Vec<&str> = d.walls.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["w2"]);
        assert!(
            d.openings.is_empty(),
            "проём демонтированной стены уходит вместе с ней"
        );
        let b = project(&doc(), Some("B"));
        assert_eq!(b.walls.len(), 1);
        assert_eq!(b.walls[0].id, "w1");
    }

    #[test]
    fn variant_cannot_reference_other_variant() {
        let mut d = doc();
        let err = wall("w3", "n1", "n3", Some("B")).apply(&mut d).unwrap_err();
        assert!(matches!(
            err,
            crate::CommandError::InvalidValue {
                field: "variant",
                ..
            }
        ));
    }

    #[test]
    fn variant_with_elements_cannot_be_removed() {
        let mut d = doc();
        let err = Command::Remove { id: "A".into() }
            .apply(&mut d)
            .unwrap_err();
        assert!(matches!(err, crate::CommandError::Referenced { .. }));
        Command::Remove { id: "B".into() }.apply(&mut d).unwrap();
    }
}
