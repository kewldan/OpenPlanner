//! Оверлей: выделение, наведение, ручки, превью инструментов, привязки.

use glam::DVec2;
use op_geom::{
    contains,
    walls::{WallInput, wall_outlines},
};
use op_model::{Document, Justification};
use op_render::{
    scene::{Color, Scene},
    text::Font,
};

use crate::{
    derived::Derived,
    draw::{self, palette, screen_text},
    geometry, hit,
    protocol::{SnapKind, Tool},
    state::EngineState,
    tools::{DragKind, ToolData},
};

/// Обводка элемента цветом. `false` — элемента нет в документе.
fn outline(
    scene: &mut Scene,
    doc: &Document,
    derived: &Derived,
    font: Option<&Font>,
    id: &str,
    color: Color,
    width: f32,
) -> bool {
    if let Some(o) = derived.wall_outlines.get(id) {
        scene.polyline(o, true, width, color);
        return true;
    }
    if let Some(o) = doc.opening(id) {
        if let Some(g) = geometry::opening(doc, derived, o) {
            scene.polyline(&g.rect, true, width, color);
            draw::opening_symbol(scene, &g, o.kind, o.hinge_at_end, o.swing, color, width);
        }
        return true;
    }
    if doc.room(id).is_some() {
        if let Some(&i) = derived.room_regions.get(id) {
            scene.polyline(&derived.regions[i].contour, true, width, color);
        }
        return true;
    }
    if let Some(d) = doc.dimension(id) {
        if let Some(g) = geometry::dimension(derived, d) {
            draw::dimension_symbol(scene, font, &g, color, width);
        }
        return true;
    }
    if let Some(l) = doc.leader(id) {
        let text = DVec2::new(l.text_x, l.text_y);
        scene.line(DVec2::new(l.x, l.y), text, width, color);
        scene.line(
            text,
            text + DVec2::X * geometry::leader_shelf(font, l),
            width,
            color,
        );
        return true;
    }
    if let Some(f) = doc.furniture_item(id) {
        scene.polyline(&crate::furniture::corners(f), true, width, color);
        return true;
    }
    if let Some(u) = doc.underlay(id) {
        let (lo, hi) = geometry::underlay_rect(u);
        scene.polyline(
            &[lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)],
            true,
            width,
            color,
        );
        return true;
    }
    false
}

fn snap_marker(scene: &mut Scene, p: DVec2, kind: Option<SnapKind>) {
    let accent = palette::accent();
    match kind {
        Some(SnapKind::Endpoint) => scene.handle(p, 11.0, accent),
        Some(SnapKind::Midpoint | SnapKind::Perpendicular) => scene.ring(p, 12.0, accent),
        Some(SnapKind::OnWall) => scene.dot(p, 7.0, accent),
        Some(SnapKind::Axis | SnapKind::Grid) => scene.dot(p, 5.0, accent),
        None => {}
    }
}

/// Контур будущей стены.
fn wall_preview(a: DVec2, b: DVec2, thickness: f64, justification: Justification) -> Vec<DVec2> {
    let (left, right) = match justification {
        Justification::Center => (thickness / 2.0, thickness / 2.0),
        Justification::Left => (thickness, 0.0),
        Justification::Right => (0.0, thickness),
    };
    wall_outlines(
        &[a, b],
        &[WallInput {
            a: 0,
            b: 1,
            left,
            right,
        }],
    )
    .into_iter()
    .next()
    .unwrap_or_default()
}

impl EngineState {
    /// Сцена оверлея.
    pub(crate) fn overlay(&self, font: Option<&Font>) -> Scene {
        let mut scene = Scene::new();
        let (doc, derived) = self.shown();
        let accent = palette::accent();
        let px = self.camera.px_per_mm;

        let survey = self.survey.as_ref();
        let mark = |scene: &mut Scene, id: &str, color: Color, width: f32| {
            if !outline(scene, doc, derived, font, id, color, width)
                && let Some((s, d)) = survey
            {
                outline(scene, s, d, font, id, color, width);
            }
        };
        if matches!(self.tool, Tool::Select | Tool::Demolish)
            && let Some(id) = &self.hover
            && !self.selection.contains(id)
            && self.drag.is_none()
        {
            let color = if self.tool == Tool::Demolish {
                palette::danger()
            } else {
                accent
            };
            mark(&mut scene, id, color.with_alpha(0.7), 2.0);
        }
        for id in &self.selection {
            mark(&mut scene, id, accent, 2.0);
        }
        if self.tool == Tool::Select && !self.drag.as_ref().is_some_and(|d| d.active) {
            for h in hit::handles(doc, derived, &self.selection) {
                if self.owns(h.kind.target()) {
                    scene.handle(h.pos, 9.0, accent);
                }
            }
        }

        if let Some(drag) = &self.drag
            && drag.active
            && matches!(drag.kind, DragKind::Marquee { .. })
        {
            let (a, b) = (drag.start_world, drag.current);
            let rect = [a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)];
            scene.fill(&[&rect], accent.with_alpha(0.08));
            for i in 0..4 {
                scene.dashed(rect[i], rect[(i + 1) % 4], 1.0, 4.0, accent);
            }
        }

        let Some(cursor) = &self.cursor else {
            return scene;
        };
        let p = cursor.point;
        for (from, to) in &cursor.guides {
            scene.dashed(*from, *to, 1.0, 4.0, accent.with_alpha(0.6));
        }

        match (&self.tool, &self.data) {
            (Tool::Wall, ToolData::Wall { anchor, .. }) => {
                let a = anchor.pos;
                if a.distance(p) > 1e-6 {
                    let shape = wall_preview(
                        a,
                        p,
                        self.options.wall_thickness,
                        self.options.justification,
                    );
                    scene.fill(&[&shape], accent.with_alpha(0.3));
                    scene.polyline(&shape, true, 1.5, accent);
                    if let Some(font) = font {
                        let (_, up) = geometry::readable((p - a).normalize());
                        let at =
                            a.midpoint(p) + up * (self.options.wall_thickness / 2.0 + 16.0 / px);
                        scene.text(
                            font,
                            at,
                            &geometry::format_length(a.distance(p)),
                            screen_text(12.0, accent),
                        );
                    }
                }
                scene.handle(a, 9.0, accent);
            }
            (Tool::Furniture, _) => {
                if let Some(f) = self.furniture_candidate(p, false) {
                    draw::furniture_symbol(
                        &mut scene,
                        &f,
                        None,
                        accent,
                        1.5,
                        Some(accent.with_alpha(0.08)),
                    );
                }
            }
            (Tool::Door | Tool::Window | Tool::Arch, _) => {
                if let Some(o) = self.opening_candidate(p, false)
                    && let Some(g) = geometry::opening(doc, derived, &o)
                {
                    scene.fill(&[&g.rect], Color::hex(0xFBFBF8, 1.0));
                    scene.polyline(&g.rect, true, 1.5, accent);
                    draw::opening_symbol(
                        &mut scene,
                        &g,
                        o.kind,
                        o.hinge_at_end,
                        o.swing,
                        accent,
                        1.5,
                    );
                }
            }
            (Tool::Room, _) => {
                if let Some(i) = op_geom::shapes::region_at(&derived.regions, p) {
                    let region = &derived.regions[i];
                    scene.fill(&[&region.contour], accent.with_alpha(0.12));
                    scene.polyline(&region.contour, true, 2.0, accent);
                }
            }
            (Tool::Dimension, ToolData::Dimension { start, end }) => {
                let (a, b, offset) = match end {
                    None => (start.0, p, 0.0),
                    Some(end) => (
                        start.0,
                        end.0,
                        self.dimension_offset(start.0, end.0, p, false),
                    ),
                };
                if a.distance(b) > 1e-6 {
                    let d = op_model::Dimension {
                        variant: None,
                        id: String::new(),
                        layer: String::new(),
                        start: op_model::Anchor::Point { x: a.x, y: a.y },
                        end: op_model::Anchor::Point { x: b.x, y: b.y },
                        offset,
                    };
                    if let Some(g) = geometry::dimension(derived, &d) {
                        draw::dimension_symbol(&mut scene, font, &g, accent, 1.5);
                    }
                }
            }
            (Tool::Leader, ToolData::Leader { target }) => {
                scene.line(*target, p, 1.5, accent);
                scene.dot(*target, 6.0, accent);
            }
            (Tool::Calibrate, ToolData::Calibrate { first, second, .. }) => {
                let b = second.unwrap_or(p);
                scene.line(*first, b, 2.0, accent);
                scene.handle(*first, 9.0, accent);
                scene.handle(b, 9.0, accent);
                if let Some(font) = font {
                    let (_, up) = geometry::readable((b - *first).normalize_or(DVec2::X));
                    scene.text(
                        font,
                        first.midpoint(b) + up * 16.0 / px,
                        &format!(
                            "{} мм на подложке",
                            geometry::format_length(first.distance(b))
                        ),
                        screen_text(12.0, accent),
                    );
                }
            }
            _ => {}
        }

        if self.tool != Tool::Select {
            snap_marker(&mut scene, p, cursor.kind);
            if matches!(self.tool, Tool::Room)
                && !derived.regions.iter().any(|r| contains(&r.contour, p))
            {
                scene.ring(p, 10.0, palette::danger().with_alpha(0.6));
            }
        }
        scene
    }
}
