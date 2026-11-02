//! Построение команд для правок на холсте: стены, удаление, перенос, слияние узлов.

use std::collections::HashSet;

use glam::DVec2;
use op_model::{Anchor, Command, Document, Element, Justification, LayerKind, Node, Wall};

use crate::{derived::Derived, ids::IdGen};

fn put(element: Element) -> Command {
    Command::Put {
        element,
        index: None,
    }
}

/// Конец новой стены: существующий узел или новая точка.
#[derive(Debug, Clone, PartialEq)]
pub struct WallEnd {
    /// Точка, мм.
    pub pos: DVec2,
    /// Существующий узел.
    pub node: Option<String>,
}

/// Команды для новой стены. Возвращает команды и идентификатор конечного узла.
pub fn new_wall(
    ids: &mut IdGen,
    start: &WallEnd,
    end: &WallEnd,
    thickness: f64,
    justification: Justification,
) -> (Vec<Command>, String) {
    let mut commands = Vec::new();
    let mut node = |end: &WallEnd, commands: &mut Vec<Command>| {
        end.node.clone().unwrap_or_else(|| {
            let id = ids.generate();
            commands.push(put(Element::Node(Node {
                variant: None,
                id: id.clone(),
                x: end.pos.x,
                y: end.pos.y,
            })));
            id
        })
    };
    let a = node(start, &mut commands);
    let b = node(end, &mut commands);
    commands.push(put(Element::Wall(Wall {
        material: None,
        variant: None,
        id: ids.generate(),
        layer: LayerKind::Walls.id().into(),
        a,
        b: b.clone(),
        thickness,
        justification,
        height: None,
    })));
    (commands, b)
}

/// Команды удаления элементов с зависимыми: проёмы стен, осиротевшие узлы,
/// файлы подложек. Размеры, привязанные к удаляемым узлам, становятся свободными.
#[must_use]
pub fn delete(doc: &Document, derived: &Derived, ids: &[String]) -> Vec<Command> {
    let selected: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let walls: HashSet<&str> = doc
        .walls
        .iter()
        .filter(|w| selected.contains(w.id.as_str()))
        .map(|w| w.id.as_str())
        .collect();
    let mut commands = Vec::new();

    for o in &doc.openings {
        if selected.contains(o.id.as_str()) || walls.contains(o.wall.as_str()) {
            commands.push(Command::Remove { id: o.id.clone() });
        }
    }
    // Узлы, у которых не останется стен.
    let orphans: HashSet<&str> = doc
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .filter(|id| {
            let used: Vec<_> = doc
                .walls
                .iter()
                .filter(|w| w.a == *id || w.b == *id)
                .collect();
            !used.is_empty() && used.iter().all(|w| walls.contains(w.id.as_str()))
        })
        .collect();
    for d in &doc.dimensions {
        if selected.contains(d.id.as_str()) {
            commands.push(Command::Remove { id: d.id.clone() });
            continue;
        }
        let free = |a: &Anchor| match a {
            Anchor::Node { node } if orphans.contains(node.as_str()) => derived
                .nodes
                .get(node)
                .map(|p| Anchor::Point { x: p.x, y: p.y }),
            _ => None,
        };
        let (start, end) = (free(&d.start), free(&d.end));
        if start.is_some() || end.is_some() {
            let mut d = d.clone();
            if let Some(s) = start {
                d.start = s;
            }
            if let Some(e) = end {
                d.end = e;
            }
            commands.push(put(Element::Dimension(d)));
        }
    }
    for w in &doc.walls {
        if walls.contains(w.id.as_str()) {
            commands.push(Command::Remove { id: w.id.clone() });
        }
    }
    for n in &doc.nodes {
        if orphans.contains(n.id.as_str()) {
            commands.push(Command::Remove { id: n.id.clone() });
        }
    }
    for r in &doc.rooms {
        if selected.contains(r.id.as_str()) {
            commands.push(Command::Remove { id: r.id.clone() });
        }
    }
    for l in &doc.leaders {
        if selected.contains(l.id.as_str()) {
            commands.push(Command::Remove { id: l.id.clone() });
        }
    }
    let mut removed_assets = HashSet::new();
    for u in &doc.underlays {
        if selected.contains(u.id.as_str()) {
            commands.push(Command::Remove { id: u.id.clone() });
            removed_assets.insert(u.asset.as_str());
        }
    }
    // Электроточки; удалённые светильники уходят из клавиш выключателей и сценариев.
    let lights: HashSet<&str> = doc
        .devices
        .iter()
        .filter(|d| selected.contains(d.id.as_str()) && d.kind.is_light())
        .map(|d| d.id.as_str())
        .collect();
    for d in &doc.devices {
        if selected.contains(d.id.as_str()) {
            commands.push(Command::Remove { id: d.id.clone() });
        } else if d.keys.iter().flatten().any(|l| lights.contains(l.as_str())) {
            let mut d = d.clone();
            for key in &mut d.keys {
                key.retain(|l| !lights.contains(l.as_str()));
            }
            commands.push(put(Element::Device(d)));
        }
    }
    if doc
        .scenes
        .iter()
        .any(|s| s.lights.iter().any(|l| lights.contains(l.as_str())))
    {
        let mut scenes = doc.scenes.clone();
        for s in &mut scenes {
            s.lights.retain(|l| !lights.contains(l.as_str()));
        }
        commands.push(Command::SetScenes { scenes });
    }
    for f in &doc.furniture {
        if selected.contains(f.id.as_str()) {
            commands.push(Command::Remove { id: f.id.clone() });
            if let Some(model) = &f.model {
                removed_assets.insert(model.as_str());
            }
        }
    }
    for asset in removed_assets {
        let still_used =
            doc.underlays
                .iter()
                .any(|u| u.asset == asset && !selected.contains(u.id.as_str()))
                || doc.furniture.iter().any(|f| {
                    f.model.as_deref() == Some(asset) && !selected.contains(f.id.as_str())
                });
        if !still_used {
            commands.push(Command::RemoveAsset {
                path: asset.to_owned(),
            });
        }
    }
    commands
}

/// Команды переноса выделения на вектор: узлы выделенных стен, помещения,
/// выноски, подложки, свободные концы размеров.
#[must_use]
pub fn translate(doc: &Document, ids: &[String], delta: DVec2) -> Vec<Command> {
    let selected: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut nodes: HashSet<&str> = HashSet::new();
    for w in &doc.walls {
        if selected.contains(w.id.as_str()) {
            nodes.insert(&w.a);
            nodes.insert(&w.b);
        }
    }
    let mut commands = Vec::new();
    for n in &doc.nodes {
        if nodes.contains(n.id.as_str()) {
            commands.push(put(Element::Node(Node {
                variant: None,
                id: n.id.clone(),
                x: n.x + delta.x,
                y: n.y + delta.y,
            })));
        }
    }
    for r in &doc.rooms {
        if selected.contains(r.id.as_str()) {
            let mut r = r.clone();
            r.x += delta.x;
            r.y += delta.y;
            commands.push(put(Element::Room(r)));
        }
    }
    for l in &doc.leaders {
        if selected.contains(l.id.as_str()) {
            let mut l = l.clone();
            l.x += delta.x;
            l.y += delta.y;
            l.text_x += delta.x;
            l.text_y += delta.y;
            commands.push(put(Element::Leader(l)));
        }
    }
    for u in &doc.underlays {
        if selected.contains(u.id.as_str()) {
            let mut u = u.clone();
            u.x += delta.x;
            u.y += delta.y;
            commands.push(put(Element::Underlay(u)));
        }
    }
    for f in &doc.furniture {
        if selected.contains(f.id.as_str()) {
            let mut f = f.clone();
            f.x += delta.x;
            f.y += delta.y;
            commands.push(put(Element::Furniture(f)));
        }
    }
    for d in &doc.devices {
        if selected.contains(d.id.as_str()) {
            let mut d = d.clone();
            d.x += delta.x;
            d.y += delta.y;
            commands.push(put(Element::Device(d)));
        }
    }
    for d in &doc.dimensions {
        if selected.contains(d.id.as_str()) {
            let shift = |a: &Anchor| match a {
                Anchor::Point { x, y } => Anchor::Point {
                    x: x + delta.x,
                    y: y + delta.y,
                },
                Anchor::Node { .. } => a.clone(),
            };
            let mut d = d.clone();
            d.start = shift(&d.start);
            d.end = shift(&d.end);
            commands.push(put(Element::Dimension(d)));
        }
    }
    commands
}

/// Команды переноса узла. Если узел брошен на другой узел — узлы сливаются:
/// ссылки переходят на целевой узел, стены нулевой длины и дубликаты удаляются.
#[must_use]
pub fn move_node(doc: &Document, node: &str, to: DVec2, merge_into: Option<&str>) -> Vec<Command> {
    let Some(target) = merge_into.filter(|t| *t != node) else {
        return vec![put(Element::Node(Node {
            variant: None,
            id: node.to_owned(),
            x: to.x,
            y: to.y,
        }))];
    };
    let mut commands = Vec::new();
    let mut kept: Vec<(String, String)> = doc
        .walls
        .iter()
        .filter(|w| w.a != node && w.b != node)
        .map(|w| (w.a.clone(), w.b.clone()))
        .collect();
    let mut removed_walls = HashSet::new();
    for w in &doc.walls {
        if w.a != node && w.b != node {
            continue;
        }
        let mut w2 = w.clone();
        if w2.a == node {
            w2.a = target.to_owned();
        }
        if w2.b == node {
            w2.b = target.to_owned();
        }
        let duplicate = kept
            .iter()
            .any(|(a, b)| (a == &w2.a && b == &w2.b) || (a == &w2.b && b == &w2.a));
        if w2.a == w2.b || duplicate {
            for o in doc.openings.iter().filter(|o| o.wall == w.id) {
                commands.push(Command::Remove { id: o.id.clone() });
            }
            removed_walls.insert(w.id.clone());
            continue;
        }
        kept.push((w2.a.clone(), w2.b.clone()));
        commands.push(put(Element::Wall(w2)));
    }
    for d in &doc.dimensions {
        let retarget = |a: &Anchor| match a {
            Anchor::Node { node: n } if n == node => Anchor::Node {
                node: target.to_owned(),
            },
            _ => a.clone(),
        };
        let (s, e) = (retarget(&d.start), retarget(&d.end));
        if s != d.start || e != d.end {
            let mut d = d.clone();
            d.start = s;
            d.end = e;
            commands.push(put(Element::Dimension(d)));
        }
    }
    for id in removed_walls {
        commands.push(Command::Remove { id });
    }
    commands.push(Command::Remove {
        id: node.to_owned(),
    });
    commands
}

/// Узлы стен, конец которых совпадает с точкой (с допуском 0,5 мм).
#[must_use]
pub fn node_at(derived: &Derived, p: DVec2) -> Option<String> {
    derived
        .nodes
        .iter()
        .find(|(_, q)| q.distance(p) < 0.5)
        .map(|(id, _)| id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(doc: &mut Document, commands: Vec<Command>) {
        Command::Batch { commands }.apply(doc).unwrap();
    }

    fn end(x: f64, y: f64, node: Option<&str>) -> WallEnd {
        WallEnd {
            pos: DVec2::new(x, y),
            node: node.map(str::to_owned),
        }
    }

    #[test]
    fn delete_wall_removes_openings_and_orphans_but_frees_dimensions() {
        let mut doc = Document::new("т");
        let mut ids = IdGen::new();
        let (c, b) = new_wall(
            &mut ids,
            &end(0.0, 0.0, None),
            &end(1000.0, 0.0, None),
            100.0,
            Justification::Center,
        );
        apply(&mut doc, c);
        let wall = doc.walls[0].clone();
        apply(
            &mut doc,
            vec![
                put(Element::Opening(op_model::Opening {
                    variant: None,
                    id: "o".into(),
                    layer: LayerKind::Openings.id().into(),
                    wall: wall.id.clone(),
                    kind: op_model::OpeningKind::Door,
                    offset: 500.0,
                    width: 800.0,
                    height: 2100.0,
                    sill: 0.0,
                    hinge_at_end: false,
                    swing: op_model::Side::Left,
                })),
                put(Element::Dimension(op_model::Dimension {
                    variant: None,
                    id: "d".into(),
                    layer: LayerKind::Dimensions.id().into(),
                    start: Anchor::Node {
                        node: wall.a.clone(),
                    },
                    end: Anchor::Node { node: b.clone() },
                    offset: 300.0,
                })),
            ],
        );
        let derived = Derived::new(&doc);
        let commands = delete(&doc, &derived, &[wall.id]);
        apply(&mut doc, commands);
        assert!(doc.walls.is_empty() && doc.openings.is_empty() && doc.nodes.is_empty());
        assert_eq!(doc.dimensions[0].end, Anchor::Point { x: 1000.0, y: 0.0 });
    }

    #[test]
    fn merging_nodes_closes_contour() {
        let mut doc = Document::new("т");
        let mut ids = IdGen::new();
        let (c, n1) = new_wall(
            &mut ids,
            &end(0.0, 0.0, None),
            &end(1000.0, 0.0, None),
            100.0,
            Justification::Center,
        );
        apply(&mut doc, c);
        let n0 = doc.walls[0].a.clone();
        let (c, _n2) = new_wall(
            &mut ids,
            &end(1000.0, 0.0, Some(&n1)),
            &end(10.0, 10.0, None),
            100.0,
            Justification::Center,
        );
        apply(&mut doc, c);
        let loose = doc.walls[1].b.clone();
        // Бросаем свободный конец второй стены на начало первой: стена-дубликат удаляется.
        let commands = move_node(&doc, &loose, DVec2::ZERO, Some(&n0));
        apply(&mut doc, commands);
        assert_eq!(doc.walls.len(), 1);
        assert_eq!(doc.nodes.len(), 2);
    }

    #[test]
    fn translate_moves_wall_nodes() {
        let mut doc = Document::new("т");
        let mut ids = IdGen::new();
        let (c, _) = new_wall(
            &mut ids,
            &end(0.0, 0.0, None),
            &end(1000.0, 0.0, None),
            100.0,
            Justification::Center,
        );
        apply(&mut doc, c);
        let id = doc.walls[0].id.clone();
        let commands = translate(&doc, &[id], DVec2::new(10.0, 20.0));
        apply(&mut doc, commands);
        assert!(DVec2::new(doc.nodes[0].x, doc.nodes[0].y).distance(DVec2::new(10.0, 20.0)) < 1e-9);
    }
}
