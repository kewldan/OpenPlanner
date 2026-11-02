//! Варианты перепланировки: правки по виду, демонтаж, копии вариантов,
//! сводка для сравнения.

use std::collections::{HashMap, HashSet};

use glam::DVec2;
use op_model::{
    Anchor, Command, Document, Element, Variant, WallMaterial,
    view::{demolished, project},
};

use crate::{
    derived::Derived,
    ids::IdGen,
    protocol::{MaterialAmount, RoomArea, ViewSummary},
};

fn put(element: Element) -> Command {
    Command::Put {
        element,
        index: None,
    }
}

fn find(doc: &Document, id: &str) -> Option<Element> {
    doc.element(id)
}

/// Может ли элемент быть элементом варианта (а не общим для проекта).
fn scoped(element: &Element) -> bool {
    !matches!(
        element,
        Element::Layer(_) | Element::Underlay(_) | Element::Variant(_)
    )
}

/// Правки в виде варианта: новые элементы получают вариант, удаление
/// элементов обмера становится демонтажом, правки обмера не применяются.
#[must_use]
pub fn adapt_variant(
    doc: &Document,
    variant: &str,
    material: Option<WallMaterial>,
    commands: Vec<Command>,
) -> Vec<Command> {
    let mut out = Vec::new();
    let mut demolish: Vec<String> = Vec::new();
    adapt_into(doc, variant, material, commands, &mut out, &mut demolish);
    // Проёмы демонтируемой стены уходят вместе с ней — отдельно их не отмечаем,
    // иначе после восстановления стены они остались бы заложенными.
    let walls: HashSet<String> = demolish
        .iter()
        .filter(|id| doc.wall(id).is_some())
        .cloned()
        .collect();
    demolish.retain(|id| doc.opening(id).is_none_or(|o| !walls.contains(&o.wall)));
    if !demolish.is_empty()
        && let Some(v) = doc.variant(variant)
    {
        let mut v = v.clone();
        for id in demolish {
            if !v.demolished.contains(&id) {
                v.demolished.push(id);
            }
        }
        out.push(put(Element::Variant(v)));
    }
    out
}

fn adapt_into(
    doc: &Document,
    variant: &str,
    material: Option<WallMaterial>,
    commands: Vec<Command>,
    out: &mut Vec<Command>,
    demolish: &mut Vec<String>,
) {
    for command in commands {
        match command {
            Command::Batch { commands } => {
                adapt_into(doc, variant, material, commands, out, demolish);
            }
            Command::Put { mut element, index } => match find(doc, element.id()) {
                // Элемент обмера в варианте не меняется.
                Some(existing) if scoped(&existing) && existing.variant().is_none() => {}
                Some(_) => out.push(Command::Put { element, index }),
                None => {
                    if scoped(&element) {
                        element.set_variant(Some(variant.to_owned()));
                        if let Element::Wall(w) = &mut element
                            && w.material.is_none()
                        {
                            w.material = material;
                        }
                    }
                    out.push(Command::Put { element, index });
                }
            },
            Command::Remove { id } => match find(doc, &id) {
                Some(existing) if scoped(&existing) && existing.variant().is_none() => {
                    // Стены, проёмы и помещения обмера демонтируются; узлы и
                    // пометки обмера остаются как есть.
                    if matches!(
                        existing,
                        Element::Wall(_)
                            | Element::Opening(_)
                            | Element::Room(_)
                            | Element::Furniture(_)
                    ) {
                        demolish.push(id);
                    }
                }
                _ => out.push(Command::Remove { id }),
            },
            other => out.push(other),
        }
    }
}

/// Правки в обмере: удаление элемента обмера убирает и то, что варианты
/// к нему пристроили (стены на его узлах, проёмы в его стенах, размеры),
/// и снимает его из списков демонтажа.
#[must_use]
pub fn adapt_survey(doc: &Document, commands: Vec<Command>) -> Vec<Command> {
    let mut removed: HashSet<String> = HashSet::new();
    collect_removed(&commands, &mut removed);
    let survey_removed: HashSet<&str> = removed
        .iter()
        .filter(|id| find(doc, id).is_some_and(|e| e.variant().is_none()))
        .map(String::as_str)
        .collect();
    if survey_removed.is_empty() {
        return commands;
    }
    let mut pre = Vec::new();
    // Стены вариантов на удаляемых узлах — и их проёмы.
    let walls: HashSet<&str> = doc
        .walls
        .iter()
        .filter(|w| {
            w.variant.is_some()
                && (survey_removed.contains(w.a.as_str()) || survey_removed.contains(w.b.as_str()))
        })
        .map(|w| w.id.as_str())
        .collect();
    for o in &doc.openings {
        if o.variant.is_some()
            && (walls.contains(o.wall.as_str()) || survey_removed.contains(o.wall.as_str()))
        {
            pre.push(Command::Remove { id: o.id.clone() });
        }
    }
    for d in &doc.dimensions {
        let on_removed = [&d.start, &d.end]
            .iter()
            .any(|a| matches!(a, Anchor::Node { node } if survey_removed.contains(node.as_str())));
        if d.variant.is_some() && on_removed {
            pre.push(Command::Remove { id: d.id.clone() });
        }
    }
    for w in walls {
        pre.push(Command::Remove { id: w.to_owned() });
    }
    for v in &doc.variants {
        if v.demolished
            .iter()
            .any(|id| survey_removed.contains(id.as_str()))
        {
            let mut v = v.clone();
            v.demolished
                .retain(|id| !survey_removed.contains(id.as_str()));
            pre.push(put(Element::Variant(v)));
        }
    }
    pre.extend(commands);
    pre
}

fn collect_removed(commands: &[Command], out: &mut HashSet<String>) {
    for command in commands {
        match command {
            Command::Remove { id } => {
                out.insert(id.clone());
            }
            Command::Batch { commands } => collect_removed(commands, out),
            _ => {}
        }
    }
}

/// Команды демонтажа или его снятия для элементов обмера.
#[must_use]
pub fn set_demolished(doc: &Document, variant: &str, ids: &[String], on: bool) -> Vec<Command> {
    let Some(v) = doc.variant(variant) else {
        return Vec::new();
    };
    let mut v = v.clone();
    for id in ids {
        let survey = find(doc, id).is_some_and(|e| {
            e.variant().is_none()
                && matches!(
                    e,
                    Element::Wall(_)
                        | Element::Opening(_)
                        | Element::Room(_)
                        | Element::Furniture(_)
                )
        });
        if !survey {
            continue;
        }
        if on && !v.demolished.contains(id) {
            v.demolished.push(id.clone());
        } else if !on {
            v.demolished.retain(|d| d != id);
        }
    }
    vec![put(Element::Variant(v))]
}

/// Новый вариант; копия переносит демонтаж и новые элементы с новыми идентификаторами.
#[must_use]
pub fn create(
    doc: &Document,
    ids: &mut IdGen,
    name: &str,
    copy_of: Option<&str>,
) -> (Vec<Command>, String) {
    let id = ids.generate();
    let source = copy_of.and_then(|c| doc.variant(c));
    let mut commands = vec![put(Element::Variant(Variant {
        id: id.clone(),
        name: name.to_owned(),
        demolished: source.map(|s| s.demolished.clone()).unwrap_or_default(),
    }))];
    let Some(source) = source else {
        return (commands, id);
    };
    let from = Some(source.id.clone());
    let mut map: HashMap<String, String> = HashMap::new();
    fn fresh(map: &mut HashMap<String, String>, old: &str, ids: &mut IdGen) -> String {
        map.entry(old.to_owned())
            .or_insert_with(|| ids.generate())
            .clone()
    }
    let re = |old: &String, map: &HashMap<String, String>| {
        map.get(old).cloned().unwrap_or_else(|| old.clone())
    };
    let mine = |v: &Option<String>| *v == from;
    let new_variant = Some(id.clone());
    for n in doc.nodes.iter().filter(|e| mine(&e.variant)) {
        let mut n = n.clone();
        n.id = fresh(&mut map, &n.id, ids);
        n.variant.clone_from(&new_variant);
        commands.push(put(Element::Node(n)));
    }
    for w in doc.walls.iter().filter(|e| mine(&e.variant)) {
        let mut w = w.clone();
        w.id = fresh(&mut map, &w.id, ids);
        w.a = re(&w.a, &map);
        w.b = re(&w.b, &map);
        w.variant.clone_from(&new_variant);
        commands.push(put(Element::Wall(w)));
    }
    for o in doc.openings.iter().filter(|e| mine(&e.variant)) {
        let mut o = o.clone();
        o.id = fresh(&mut map, &o.id, ids);
        o.wall = re(&o.wall, &map);
        o.variant.clone_from(&new_variant);
        commands.push(put(Element::Opening(o)));
    }
    for r in doc.rooms.iter().filter(|e| mine(&e.variant)) {
        let mut r = r.clone();
        r.id = fresh(&mut map, &r.id, ids);
        r.variant.clone_from(&new_variant);
        commands.push(put(Element::Room(r)));
    }
    for d in doc.dimensions.iter().filter(|e| mine(&e.variant)) {
        let mut d = d.clone();
        d.id = fresh(&mut map, &d.id, ids);
        for a in [&mut d.start, &mut d.end] {
            if let Anchor::Node { node } = a {
                *node = re(node, &map);
            }
        }
        d.variant.clone_from(&new_variant);
        commands.push(put(Element::Dimension(d)));
    }
    for l in doc.leaders.iter().filter(|e| mine(&e.variant)) {
        let mut l = l.clone();
        l.id = fresh(&mut map, &l.id, ids);
        l.variant.clone_from(&new_variant);
        commands.push(put(Element::Leader(l)));
    }
    for f in doc.furniture.iter().filter(|e| mine(&e.variant)) {
        let mut f = f.clone();
        f.id = fresh(&mut map, &f.id, ids);
        f.variant.clone_from(&new_variant);
        commands.push(put(Element::Furniture(f)));
    }
    (commands, id)
}

/// Команды удаления варианта вместе с его элементами.
#[must_use]
pub fn delete(doc: &Document, variant: &str) -> Vec<Command> {
    let mine = |v: &Option<String>| v.as_deref() == Some(variant);
    let mut out = Vec::new();
    let mut remove = |id: &str| out.push(Command::Remove { id: id.to_owned() });
    doc.openings
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    doc.dimensions
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    doc.leaders
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    doc.furniture
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    doc.rooms
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    doc.walls
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    doc.nodes
        .iter()
        .filter(|e| mine(&e.variant))
        .for_each(|e| remove(&e.id));
    out.push(Command::Remove {
        id: variant.to_owned(),
    });
    out
}

/// Название вида.
#[must_use]
pub fn view_name(doc: &Document, variant: Option<&str>) -> String {
    variant
        .and_then(|v| doc.variant(v))
        .map_or_else(|| "Обмер (как есть)".to_owned(), |v| v.name.clone())
}

/// Сводка вида: площади помещений и изменения относительно обмера.
#[must_use]
pub fn summary(doc: &Document, variant: Option<&str>) -> ViewSummary {
    let projected = project(doc, variant);
    let derived = Derived::new(&projected);
    // Метки в одной области (снесли перегородку) — одно помещение: площадь у первой,
    // остальные считаются слитыми и в список не входят.
    let mut used = HashSet::new();
    let rooms: Vec<RoomArea> = projected
        .rooms
        .iter()
        .filter(|r| {
            derived
                .room_regions
                .get(&r.id)
                .is_none_or(|&i| used.insert(i))
        })
        .map(|r| RoomArea {
            id: r.id.clone(),
            name: r.name.clone(),
            area: derived
                .room_regions
                .get(&r.id)
                .map_or(0.0, |&i| op_geom::mm2_to_m2(derived.regions[i].area)),
        })
        .collect();
    let total_area = rooms.iter().map(|r| r.area).sum();
    let height = |h: Option<f64>| h.unwrap_or(doc.default_ceiling_height) / 1000.0;
    let survey = project(doc, None);
    let survey_derived = Derived::new(&survey);
    let removed = variant.map(|v| demolished(doc, v)).unwrap_or_default();
    let (mut demolished_length, mut demolished_area) = (0.0, 0.0);
    for w in survey.walls.iter().filter(|w| removed.contains(&w.id)) {
        if let Some((a, b)) = survey_derived.wall_axis(w) {
            let len = a.distance(b) / 1000.0;
            demolished_length += len;
            demolished_area += len * height(w.height);
        }
    }
    let mut new_walls: Vec<MaterialAmount> = Vec::new();
    for w in projected.walls.iter().filter(|w| w.variant.is_some()) {
        let Some((a, b)) = derived.wall_axis(w) else {
            continue;
        };
        let len = a.distance(b) / 1000.0;
        match new_walls.iter_mut().find(|m| m.material == w.material) {
            Some(m) => {
                m.length += len;
                m.area += len * height(w.height);
            }
            None => new_walls.push(MaterialAmount {
                material: w.material,
                length: len,
                area: len * height(w.height),
            }),
        }
    }
    let demolished_walls: HashSet<&str> = doc
        .variant(variant.unwrap_or(""))
        .map(|v| v.demolished.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let openings_closed = survey
        .openings
        .iter()
        .filter(|o| removed.contains(&o.id) && !demolished_walls.contains(o.wall.as_str()))
        .count();
    ViewSummary {
        variant: variant.map(str::to_owned),
        name: view_name(doc, variant),
        rooms,
        total_area,
        demolished_length,
        demolished_area,
        new_walls,
        openings_added: u32::try_from(
            projected
                .openings
                .iter()
                .filter(|o| o.variant.is_some())
                .count(),
        )
        .unwrap_or(0),
        openings_closed: u32::try_from(openings_closed).unwrap_or(0),
    }
}

/// Сдвигает документ на вектор (для показа двух видов рядом).
pub fn translate(doc: &mut Document, d: DVec2) {
    for n in &mut doc.nodes {
        n.x += d.x;
        n.y += d.y;
    }
    for r in &mut doc.rooms {
        r.x += d.x;
        r.y += d.y;
    }
    for l in &mut doc.leaders {
        l.x += d.x;
        l.y += d.y;
        l.text_x += d.x;
        l.text_y += d.y;
    }
    for u in &mut doc.underlays {
        u.x += d.x;
        u.y += d.y;
    }
    for f in &mut doc.furniture {
        f.x += d.x;
        f.y += d.y;
    }
    for dim in &mut doc.dimensions {
        for a in [&mut dim.start, &mut dim.end] {
            if let Anchor::Point { x, y } = a {
                *x += d.x;
                *y += d.y;
            }
        }
    }
}
