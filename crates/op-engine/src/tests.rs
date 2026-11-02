//! Сценарии движка: ввод указателем и клавиатурой, как из UI.

use glam::DVec2;
use op_format::journal;
use op_model::{Anchor, Command, Element, LayerKind};

use crate::{
    EngineState, UNTITLED,
    protocol::{EngineEvents, FromEngine, PointerPhase, ToEngine, Tool, ToolStep},
};

fn state() -> EngineState {
    EngineState::new(DVec2::new(1000.0, 800.0), 1.0)
}

fn screen(s: &EngineState, world: DVec2) -> DVec2 {
    s.camera().world_to_screen(world, s.viewport())
}

fn pointer(s: &mut EngineState, phase: PointerPhase, world: DVec2, shift: bool) -> EngineEvents {
    let p = screen(s, world);
    s.handle(ToEngine::Pointer {
        phase,
        x: p.x,
        y: p.y,
        shift,
        alt: false,
    })
}

fn click(s: &mut EngineState, x: f64, y: f64) -> EngineEvents {
    let p = DVec2::new(x, y);
    pointer(s, PointerPhase::Move, p, false);
    let mut events = pointer(s, PointerPhase::Down, p, false);
    events
        .events
        .extend(pointer(s, PointerPhase::Up, p, false).events);
    events
}

fn drag(s: &mut EngineState, from: DVec2, to: DVec2) -> EngineEvents {
    pointer(s, PointerPhase::Down, from, false);
    pointer(s, PointerPhase::Move, from.lerp(to, 0.5), false);
    pointer(s, PointerPhase::Move, to, false);
    pointer(s, PointerPhase::Up, to, false)
}

fn tool(s: &mut EngineState, tool: Tool) {
    s.handle(ToEngine::SetTool { tool });
}

fn has_error(events: &EngineEvents) -> bool {
    events
        .events
        .iter()
        .any(|e| matches!(e, FromEngine::Error { .. }))
}

/// Прямоугольник стен по осям 3000 × 4000 мм от точки (1000, 1000).
fn rectangle(s: &mut EngineState) {
    tool(s, Tool::Wall);
    for (x, y) in [
        (1000.0, 1000.0),
        (4000.0, 1000.0),
        (4000.0, 5000.0),
        (1000.0, 5000.0),
        (1000.0, 1000.0),
    ] {
        assert!(!has_error(&click(s, x, y)));
    }
}

fn room_area(events: &EngineEvents) -> Option<f64> {
    events.events.iter().find_map(|e| match e {
        FromEngine::DerivedChanged { rooms, .. } => rooms.first().map(|r| r.area),
        _ => None,
    })
}

#[test]
fn draws_closed_chain_and_finds_room() {
    let mut s = state();
    rectangle(&mut s);
    let doc = s.document();
    assert_eq!(doc.walls.len(), 4);
    assert_eq!(doc.nodes.len(), 4, "цепочка замкнулась на первом узле");
    assert_eq!(s.step(), ToolStep::WallStart, "замыкание завершает цепочку");

    let events = s.handle(ToEngine::FindRooms);
    let area = room_area(&events).unwrap();
    // Толщина 120 мм по оси: площадь по внутренним поверхностям.
    assert!((area - 2.88 * 3.88).abs() < 1e-6, "{area}");
    assert_eq!(s.document().rooms[0].name, "Помещение 1");
    // Повторно контуров нет.
    assert!(has_error(&s.handle(ToEngine::FindRooms)));
}

#[test]
fn numeric_length_and_angle() {
    let mut s = state();
    tool(&mut s, Tool::Wall);
    click(&mut s, 1000.0, 1000.0);
    pointer(
        &mut s,
        PointerPhase::Move,
        DVec2::new(2000.0, 1000.0),
        false,
    );
    s.handle(ToEngine::NumericInput {
        value: 3500.0,
        angle: None,
    });
    s.handle(ToEngine::NumericInput {
        value: 2000.0,
        angle: Some(90.0),
    });
    let doc = s.document();
    assert_eq!(doc.walls.len(), 2);
    let last = doc.node(&doc.walls[1].b).unwrap();
    assert!(
        (last.x - 4500.0).abs() < 1e-6 && (last.y + 1000.0).abs() < 1e-6,
        "{last:?}"
    );
    assert_eq!(s.step(), ToolStep::WallNext);
    s.handle(ToEngine::Finish);
    assert_eq!(s.step(), ToolStep::WallStart);
    s.handle(ToEngine::Cancel);
    assert!(s.snapshot().events.iter().any(|e| matches!(
        e,
        FromEngine::ToolChanged {
            tool: Tool::Select,
            ..
        }
    )));
}

#[test]
fn each_segment_is_one_undo_step() {
    let mut s = state();
    rectangle(&mut s);
    s.handle(ToEngine::Undo);
    assert_eq!(s.document().walls.len(), 3);
    s.handle(ToEngine::Redo);
    assert_eq!(s.document().walls.len(), 4);
}

#[test]
fn t_junction_divides_room() {
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Wall);
    // Перегородка от середины верхней стены к середине нижней.
    click(&mut s, 2500.0, 1000.0);
    click(&mut s, 2500.0, 5000.0);
    s.handle(ToEngine::Finish);
    let events = s.handle(ToEngine::FindRooms);
    let rooms = events
        .events
        .iter()
        .find_map(|e| match e {
            FromEngine::DerivedChanged { rooms, .. } => Some(rooms.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(rooms.len(), 2);
    for r in rooms {
        assert!((r.area - 1.38 * 3.88).abs() < 1e-6, "{}", r.area);
    }
}

#[test]
fn openings_are_placed_on_walls() {
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Door);
    assert!(!has_error(&click(&mut s, 2000.0, 1030.0)));
    let o = &s.document().openings[0];
    assert!((o.offset - 1000.0).abs() < 1e-6, "{}", o.offset);
    assert_eq!(s.selection(), std::slice::from_ref(&o.id));
    // Второй проём поверх первого — ошибка.
    assert!(has_error(&click(&mut s, 2100.0, 1000.0)));
    // Мимо стены — ошибка.
    assert!(has_error(&click(&mut s, 2500.0, 3000.0)));
    // Окно по центру стены притягивается к середине.
    tool(&mut s, Tool::Window);
    click(&mut s, 2540.0, 5000.0);
    let w = &s.document().openings[1];
    assert!((w.offset - 1500.0).abs() < 1e-6, "{}", w.offset);
    // Проём не размыкает помещение.
    let events = s.handle(ToEngine::FindRooms);
    assert!(room_area(&events).is_some());
}

#[test]
fn room_tool_needs_closed_contour() {
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Room);
    assert!(has_error(&click(&mut s, 9000.0, 9000.0)));
    let events = click(&mut s, 2500.0, 3000.0);
    assert!(room_area(&events).is_some());
    // Щелчок в то же помещение выделяет его, а не создаёт второе.
    click(&mut s, 3000.0, 4000.0);
    assert_eq!(s.document().rooms.len(), 1);
    assert_eq!(s.selection(), [s.document().rooms[0].id.clone()]);
}

#[test]
fn dimension_follows_wall_nodes() {
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Dimension);
    click(&mut s, 1000.0, 1000.0);
    click(&mut s, 4000.0, 1000.0);
    click(&mut s, 2500.0, 500.0);
    let d = s.document().dimensions[0].clone();
    assert!(matches!(d.start, Anchor::Node { .. }) && matches!(d.end, Anchor::Node { .. }));
    assert!((d.offset - 500.0).abs() < 1e-6, "{}", d.offset);

    let wall = s
        .document()
        .walls
        .iter()
        .find(|w| {
            s.document()
                .node(&w.b)
                .is_some_and(|n| (n.x - 4000.0).abs() < 1e-6 && (n.y - 1000.0).abs() < 1e-6)
        })
        .unwrap()
        .id
        .clone();
    let events = s.handle(ToEngine::SetWallLength {
        id: wall,
        length: 3600.0,
    });
    let length = events
        .events
        .iter()
        .find_map(|e| match e {
            FromEngine::DerivedChanged { dimensions, .. } => dimensions.first().map(|d| d.length),
            _ => None,
        })
        .unwrap();
    assert!((length - 3600.0).abs() < 1e-6, "{length}");
}

#[test]
fn drag_node_handle_and_merge() {
    let mut s = state();
    tool(&mut s, Tool::Wall);
    click(&mut s, 1000.0, 1000.0);
    click(&mut s, 3000.0, 1000.0);
    click(&mut s, 3000.0, 3000.0);
    s.handle(ToEngine::Finish);
    tool(&mut s, Tool::Select);
    click(&mut s, 3000.0, 2000.0);
    assert_eq!(s.selection().len(), 1);
    // Тянем конец второй стены на начало первой: узлы сливаются, контур — треугольник.
    drag(
        &mut s,
        DVec2::new(3000.0, 3000.0),
        DVec2::new(1000.0, 1000.0),
    );
    assert_eq!(s.document().nodes.len(), 2, "{:?}", s.document().nodes);
    assert_eq!(s.document().walls.len(), 1, "стена-дубликат удалена");
    s.handle(ToEngine::Undo);
    assert_eq!(s.document().nodes.len(), 3);
}

#[test]
fn drag_moves_selected_wall_with_snapping() {
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Select);
    click(&mut s, 2500.0, 1000.0);
    drag(
        &mut s,
        DVec2::new(2500.0, 1000.0),
        DVec2::new(2520.0, 1480.0),
    );
    let ys: Vec<f64> = s.document().nodes.iter().map(|n| n.y).collect();
    assert!(ys.iter().any(|y| (y - 1500.0).abs() < 1e-6), "{ys:?}");
    assert_eq!(s.history_len_for_tests(), 5, "четыре стены и один перенос");
}

#[test]
fn marquee_and_delete_cascade() {
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Door);
    click(&mut s, 2000.0, 1000.0);
    tool(&mut s, Tool::Select);
    pointer(&mut s, PointerPhase::Down, DVec2::new(500.0, 500.0), false);
    pointer(
        &mut s,
        PointerPhase::Move,
        DVec2::new(4500.0, 1500.0),
        false,
    );
    pointer(&mut s, PointerPhase::Up, DVec2::new(4500.0, 1500.0), false);
    assert_eq!(s.selection().len(), 2, "верхняя стена и дверь");
    s.handle(ToEngine::DeleteSelection);
    let doc = s.document();
    assert_eq!(doc.walls.len(), 3);
    assert!(doc.openings.is_empty());
    assert_eq!(doc.nodes.len(), 4, "узлы остальных стен на месте");
    assert!(s.selection().is_empty());
}

#[test]
fn hidden_and_locked_layers() {
    let mut s = state();
    rectangle(&mut s);
    let mut layer = s.document().layer(LayerKind::Walls.id()).unwrap().clone();
    layer.locked = true;
    s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::Layer(layer.clone()),
            index: None,
        },
    });
    tool(&mut s, Tool::Select);
    click(&mut s, 2500.0, 1000.0);
    assert!(s.selection().is_empty(), "заблокированное не выделяется");

    let (doc_scene, _) = s.take_scenes();
    assert!(!doc_scene.unwrap().fill_indices.is_empty());
    layer.visible = false;
    s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::Layer(layer),
            index: None,
        },
    });
    let (doc_scene, _) = s.take_scenes();
    assert!(
        doc_scene.unwrap().fill_indices.is_empty(),
        "скрытые стены не рисуются"
    );
}

#[test]
fn leader_asks_for_text() {
    let mut s = state();
    tool(&mut s, Tool::Leader);
    click(&mut s, 1000.0, 1000.0);
    let events = click(&mut s, 2000.0, 500.0);
    let id = s.document().leaders[0].id.clone();
    assert!(events.events.contains(&FromEngine::EditText { id }));
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn underlay_calibration_file_and_journal() {
    let mut s = state();
    let header = journal::header_line().unwrap();
    let mut log = header.clone();
    let record = |events: &EngineEvents, log: &mut String| {
        for e in &events.events {
            if let FromEngine::Journal { line } = e {
                log.push_str(line);
            }
        }
    };
    let events = s.add_underlay(png(100, 50), "план.png").unwrap();
    record(&events, &mut log);
    let u = s.document().underlays[0].clone();
    assert_eq!((u.width_px, u.height_px), (100, 50));
    assert_eq!(s.selection(), std::slice::from_ref(&u.id));
    assert_eq!(s.take_image_uploads().len(), 1);
    assert!(
        s.take_image_uploads().is_empty(),
        "изображение загружается один раз"
    );

    // Калибровка: две точки на расстоянии 10 px изображения = 10·k мм, настоящее — 1000 мм.
    tool(&mut s, Tool::Calibrate);
    let a = DVec2::new(u.x, u.y) + DVec2::new(10.0, 10.0) * u.mm_per_px;
    let b = a + DVec2::new(10.0, 0.0) * u.mm_per_px;
    let p = |s: &EngineState, w: DVec2| screen(s, w);
    for w in [a, b] {
        let sp = p(&s, w);
        s.handle(ToEngine::Pointer {
            phase: PointerPhase::Down,
            x: sp.x,
            y: sp.y,
            shift: false,
            alt: true,
        });
    }
    assert_eq!(s.step(), ToolStep::CalibrateDistance);
    let events = s.handle(ToEngine::NumericInput {
        value: 1000.0,
        angle: None,
    });
    record(&events, &mut log);
    let calibrated = s.document().underlays[0].clone();
    assert!(
        (calibrated.mm_per_px - 100.0).abs() < 1e-6,
        "{}",
        calibrated.mm_per_px
    );

    // Файл: подложка и её изображение сохраняются.
    let bytes = s.export_file("test").unwrap();
    let mut other = state();
    other.import_file(&bytes).unwrap();
    assert_eq!(other.document().underlays, s.document().underlays);
    assert_eq!(other.document().assets.len(), 1);
    assert_eq!(other.take_image_uploads().len(), 1);

    // Журнал восстанавливает подложку вместе с изображением.
    let mut recovered = state();
    recovered.replay_journal(&log).unwrap();
    assert_eq!(recovered.document().underlays, s.document().underlays);
    assert_eq!(recovered.document().assets, s.document().assets);
}

#[test]
fn bad_image_is_rejected() {
    let mut s = state();
    assert!(s.add_underlay(b"not an image".to_vec(), "x.png").is_err());
    assert!(s.document().underlays.is_empty());
}

#[test]
fn scenes_contain_walls_and_labels() {
    let mut s = state();
    rectangle(&mut s);
    s.handle(ToEngine::FindRooms);
    let (doc, overlay) = s.take_scenes();
    let doc = doc.unwrap();
    assert!(!doc.fill_indices.is_empty());
    assert!(!doc.glyphs.is_empty(), "подпись помещения");
    assert!(overlay.is_some());
    let (doc, overlay) = s.take_scenes();
    assert!(
        doc.is_none() && overlay.is_none(),
        "без изменений сцены не пересобираются"
    );
}

#[test]
fn zoom_to_fit_frames_content() {
    let mut s = state();
    rectangle(&mut s);
    s.handle(ToEngine::ZoomToFit);
    let c = s.camera().center;
    assert!(c.distance(DVec2::new(2500.0, 3000.0)) < 1e-6, "{c}");
}

// ---------- Протокол и история (стадия 0) ----------

fn rename(s: &mut EngineState, title: &str) -> EngineEvents {
    s.handle(ToEngine::Command {
        command: Command::SetTitle {
            title: title.into(),
        },
    })
}

fn title(events: &EngineEvents) -> &str {
    events
        .events
        .iter()
        .find_map(|e| match e {
            FromEngine::DocumentChanged { document } => Some(document.meta.title.as_str()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn command_then_undo() {
    let mut s = state();
    let events = rename(&mut s, "Комендантский пр.");
    assert!(events.events.contains(&FromEngine::HistoryChanged {
        can_undo: true,
        can_redo: false,
        dirty: true
    }));
    let events = s.handle(ToEngine::Undo);
    assert_eq!(title(&events), UNTITLED);
}

#[test]
fn invalid_command_reports_error() {
    let mut s = state();
    let events = s.handle(ToEngine::Command {
        command: Command::SetDefaultCeilingHeight { height: 0.0 },
    });
    assert!(matches!(
        events.events.as_slice(),
        [FromEngine::Error { .. }]
    ));
}

#[test]
fn nothing_to_undo_emits_nothing() {
    assert!(state().handle(ToEngine::Undo).events.is_empty());
}

#[test]
fn zoom_to_scale() {
    let mut s = state();
    let events = s.handle(ToEngine::ZoomToScale { denominator: 50.0 });
    let [FromEngine::ViewChanged { zoom_percent, .. }] = events.events.as_slice() else {
        panic!("ожидалось ViewChanged");
    };
    assert!((zoom_percent - 100.0).abs() < 1e-9);
}

#[test]
fn mark_saved_clears_dirty() {
    let mut s = state();
    rename(&mut s, "А");
    let events = s.handle(ToEngine::MarkSaved);
    assert_eq!(
        events.events,
        vec![FromEngine::HistoryChanged {
            can_undo: true,
            can_redo: false,
            dirty: false
        }]
    );
}

#[test]
fn journal_replay_restores_walls() {
    let mut s = state();
    let mut log = journal::header_line().unwrap();
    tool(&mut s, Tool::Wall);
    for (x, y) in [(0.0, 0.0), (2000.0, 0.0), (2000.0, 2000.0)] {
        for e in click(&mut s, x, y).events {
            if let FromEngine::Journal { line } = e {
                log += &line;
            }
        }
    }
    let mut recovered = state();
    recovered.replay_journal(&log).unwrap();
    assert_eq!(recovered.document().walls, s.document().walls);
    assert_eq!(recovered.document().nodes, s.document().nodes);
}

#[test]
fn every_element_survives_json() {
    // Журнал хранит команды в JSON: каждый вид элемента должен проходить круг без потерь.
    let mut s = state();
    rectangle(&mut s);
    tool(&mut s, Tool::Door);
    click(&mut s, 2000.0, 1000.0);
    tool(&mut s, Tool::Room);
    click(&mut s, 2500.0, 3000.0);
    tool(&mut s, Tool::Dimension);
    click(&mut s, 1000.0, 1000.0);
    click(&mut s, 4000.0, 1000.0);
    click(&mut s, 2500.0, 600.0);
    tool(&mut s, Tool::Leader);
    click(&mut s, 2000.0, 2000.0);
    click(&mut s, 2500.0, 1500.0);
    s.add_underlay(png(4, 4), "п.png").unwrap();
    s.handle(ToEngine::FurnitureTool {
        item: "bed-double".into(),
    });
    click(&mut s, 2500.0, 3000.0);
    s.handle(ToEngine::DeviceTool {
        kind: op_model::DeviceKind::Switch2,
    });
    click(&mut s, 1500.0, 1100.0);
    s.handle(ToEngine::PlumbingTool {
        item: "riser-drain".into(),
    });
    click(&mut s, 3700.0, 4700.0);
    s.handle(ToEngine::PipeTool {
        system: op_model::PipeSystem::Drain,
        diameter: 50.0,
    });
    click(&mut s, 2000.0, 4700.0);
    click(&mut s, 3700.0, 4700.0);
    s.handle(ToEngine::Finish);
    let doc = s.document();
    let elements: Vec<Element> = doc
        .layers
        .iter()
        .cloned()
        .map(Element::Layer)
        .chain(doc.nodes.iter().cloned().map(Element::Node))
        .chain(doc.walls.iter().cloned().map(Element::Wall))
        .chain(doc.openings.iter().cloned().map(Element::Opening))
        .chain(doc.rooms.iter().cloned().map(Element::Room))
        .chain(doc.dimensions.iter().cloned().map(Element::Dimension))
        .chain(doc.leaders.iter().cloned().map(Element::Leader))
        .chain(doc.underlays.iter().cloned().map(Element::Underlay))
        .chain(doc.furniture.iter().cloned().map(Element::Furniture))
        .chain(doc.devices.iter().cloned().map(Element::Device))
        .chain(doc.plumbing.iter().cloned().map(Element::PlumbingPoint))
        .chain(doc.pipes.iter().cloned().map(Element::Pipe))
        .collect();
    assert_eq!(
        elements.len(),
        10 + 4 + 4 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1
    );
    for element in elements {
        let command = Command::Put {
            element,
            index: None,
        };
        let json = serde_json::to_string(&command).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, command, "{json}");
    }
}

// ---------- Перепланировка и варианты ----------

/// Две комнаты: прямоугольник и перегородка посередине (x = 2500).
fn two_rooms(s: &mut EngineState) {
    rectangle(s);
    tool(s, Tool::Wall);
    click(s, 2500.0, 1000.0);
    click(s, 2500.0, 5000.0);
    s.handle(ToEngine::Finish);
    s.handle(ToEngine::FindRooms);
    tool(s, Tool::Select);
}

fn variant_id(s: &EngineState) -> String {
    match s.view.clone() {
        crate::protocol::View::Variant { id } => id,
        other => panic!("ожидался вариант, а не {other:?}"),
    }
}

fn summaries(events: &EngineEvents) -> Vec<crate::protocol::ViewSummary> {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::VariantsSummary { views } => Some(views.clone()),
            _ => None,
        })
        .unwrap()
}

fn partition(s: &EngineState) -> String {
    s.document()
        .walls
        .iter()
        .find(|w| {
            let d = s.document();
            let (a, b) = (d.node(&w.a).unwrap(), d.node(&w.b).unwrap());
            (a.x - 2500.0).abs() < 1.0 && (b.x - 2500.0).abs() < 1.0
        })
        .unwrap()
        .id
        .clone()
}

#[test]
fn variant_demolishes_and_adds_partitions() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "Вариант А".into(),
        copy_of: None,
    });
    let v = variant_id(&s);

    // Delete в варианте не удаляет стену обмера, а демонтирует её.
    let wall = partition(&s);
    s.handle(ToEngine::Select {
        ids: vec![wall.clone()],
    });
    let events = s.handle(ToEngine::DeleteSelection);
    assert!(s.document().wall(&wall).is_some(), "обмер не меняется");
    assert_eq!(
        s.document().variant(&v).unwrap().demolished,
        vec![wall.clone()]
    );
    let views = summaries(&events);
    assert!(
        (views[1].demolished_length - 4.0).abs() < 1e-6,
        "{:?}",
        views[1]
    );
    assert!((views[1].demolished_area - 10.8).abs() < 1e-6);

    // Новая перегородка из ГКЛ — только в варианте.
    s.handle(ToEngine::SetToolOptions {
        options: crate::protocol::ToolOptions {
            wall_thickness: 100.0,
            justification: op_model::Justification::Center,
            material: Some(op_model::WallMaterial::Gypsum),
        },
    });
    tool(&mut s, Tool::Wall);
    click(&mut s, 1000.0, 2000.0);
    let events = click(&mut s, 4000.0, 2000.0);
    let new = s.document().walls.last().unwrap().clone();
    assert_eq!(new.variant.as_deref(), Some(v.as_str()));
    assert_eq!(new.material, Some(op_model::WallMaterial::Gypsum));
    let views = summaries(&events);
    assert!(views[0].new_walls.is_empty(), "обмер без новых стен");
    assert!(
        (views[1].new_walls[0].length - 3.0).abs() < 1e-6,
        "{:?}",
        views[1].new_walls
    );
    // В варианте комнаты по-новому: перегородка демонтирована, новая — поперёк;
    // подписи помещений обмера попали в нижнюю часть, верхняя получает новое помещение.
    s.handle(ToEngine::Cancel);
    let events = s.handle(ToEngine::FindRooms);
    assert!(!has_error(&events));
    let views = summaries(&events);
    let total: f64 = views[1].rooms.iter().map(|r| r.area).sum();
    assert!(
        views[1].rooms.iter().all(|r| r.area > 0.0),
        "{:?}",
        views[1].rooms
    );
    // Две подписи обмера в одной области — одно помещение, площадь не задваивается.
    let survey_total = views[0].total_area;
    assert!(
        (total - views[1].total_area).abs() < 1e-9 && (total - survey_total).abs() < 0.2,
        "{total} vs {survey_total}: {:?}",
        views[1].rooms
    );

    // В обмере новой стены нет, перегородка на месте.
    s.handle(ToEngine::SetView {
        view: crate::protocol::View::Survey,
    });
    assert!(s.doc().wall(&new.id).is_none());
    assert!(s.doc().wall(&wall).is_some());
}

#[test]
fn survey_edits_reach_all_variants() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    s.handle(ToEngine::CreateVariant {
        name: "Б".into(),
        copy_of: None,
    });
    s.handle(ToEngine::SetView {
        view: crate::protocol::View::Survey,
    });
    let wall = partition(&s);
    // Двигаем перегородку в обмере: оба варианта видят новое положение.
    let w = s.doc().wall(&wall).unwrap().clone();
    s.handle(ToEngine::Select {
        ids: vec![wall.clone()],
    });
    let mut moved = Vec::new();
    crate::edit::translate(s.doc(), std::slice::from_ref(&wall), DVec2::new(300.0, 0.0))
        .into_iter()
        .for_each(|c| moved.push(c));
    s.handle(ToEngine::Command {
        command: Command::batch(moved),
    });
    for v in s.document().variants.clone() {
        s.handle(ToEngine::SetView {
            view: crate::protocol::View::Variant { id: v.id },
        });
        let a = s.doc().node(&w.a).unwrap();
        assert!((a.x - 2800.0).abs() < 1e-6);
    }
}

#[test]
fn survey_deletion_cascades_into_variants() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    let v = variant_id(&s);
    let wall = partition(&s);
    s.handle(ToEngine::SetDemolished {
        ids: vec![wall.clone()],
        demolished: true,
    });
    // Новая стена варианта от узла перегородки обмера.
    let node = s.document().wall(&wall).unwrap().a.clone();
    let n = s.document().node(&node).unwrap().clone();
    tool(&mut s, Tool::Wall);
    click(&mut s, n.x, n.y);
    click(&mut s, n.x + 1000.0, n.y + 1000.0);
    s.handle(ToEngine::Finish);
    let variant_walls = |s: &EngineState| {
        s.document()
            .walls
            .iter()
            .filter(|w| w.variant.is_some())
            .count()
    };
    assert_eq!(variant_walls(&s), 1);

    s.handle(ToEngine::SetView {
        view: crate::protocol::View::Survey,
    });
    s.handle(ToEngine::Select {
        ids: vec![wall.clone()],
    });
    let events = s.handle(ToEngine::DeleteSelection);
    assert!(!has_error(&events), "{:?}", events.events);
    assert!(s.document().wall(&wall).is_none());
    assert_eq!(
        variant_walls(&s),
        0,
        "стена варианта на удалённом узле удалена"
    );
    assert!(s.document().variant(&v).unwrap().demolished.is_empty());
    s.handle(ToEngine::Undo);
    assert_eq!(variant_walls(&s), 1);
}

#[test]
fn demolish_tool_toggles_and_copies_survive() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    let a = variant_id(&s);
    tool(&mut s, Tool::Demolish);
    click(&mut s, 2500.0, 2000.0);
    let wall = partition(&s);
    assert_eq!(
        s.document().variant(&a).unwrap().demolished,
        vec![wall.clone()]
    );
    // Копия варианта переносит демонтаж; повторный щелчок в копии снимает его только там.
    s.handle(ToEngine::CreateVariant {
        name: "Копия".into(),
        copy_of: Some(a.clone()),
    });
    let b = variant_id(&s);
    tool(&mut s, Tool::Demolish);
    click(&mut s, 2500.0, 2000.0);
    assert!(s.document().variant(&b).unwrap().demolished.is_empty());
    assert_eq!(s.document().variant(&a).unwrap().demolished, vec![wall]);
    // Удаление варианта.
    s.handle(ToEngine::DeleteVariant { id: b });
    assert_eq!(s.document().variants.len(), 1);
    assert_eq!(s.view, crate::protocol::View::Survey);
}

#[test]
fn compare_view_shows_both_and_is_read_only() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    let a = variant_id(&s);
    s.handle(ToEngine::SetDemolished {
        ids: vec![partition(&s)],
        demolished: true,
    });
    s.take_scenes();
    let camera = s.camera;
    s.handle(ToEngine::SetView {
        view: crate::protocol::View::Compare {
            left: None,
            right: Some(a.clone()),
        },
    });
    assert_ne!(s.camera, camera, "оба вида целиком");
    let (doc, _) = s.take_scenes();
    let doc = doc.unwrap();
    assert!(doc.glyphs.len() > 20, "подписи видов и помещений");
    assert!(has_error(&s.handle(ToEngine::SetTool { tool: Tool::Wall })));
    let before = s.document().clone();
    click(&mut s, 2000.0, 2000.0);
    assert_eq!(s.document(), &before);

    // Выход из сравнения возвращает камеру.
    s.handle(ToEngine::SetView {
        view: crate::protocol::View::Variant { id: a },
    });
    assert_eq!(s.camera, camera);
}

#[test]
fn undo_variant_creation_returns_to_survey() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    s.handle(ToEngine::Undo);
    assert_eq!(s.view, crate::protocol::View::Survey);
    assert!(s.document().variants.is_empty());
}

#[test]
fn variants_survive_file_and_journal() {
    let mut s = state();
    let mut log = journal::header_line().unwrap();
    let record = |events: EngineEvents, log: &mut String| {
        for e in events.events {
            if let FromEngine::Journal { line } = e {
                log.push_str(&line);
            }
        }
    };
    tool(&mut s, Tool::Wall);
    for (x, y) in [
        (1000.0, 1000.0),
        (4000.0, 1000.0),
        (4000.0, 5000.0),
        (1000.0, 5000.0),
        (1000.0, 1000.0),
    ] {
        record(click(&mut s, x, y), &mut log);
    }
    record(
        s.handle(ToEngine::CreateVariant {
            name: "А".into(),
            copy_of: None,
        }),
        &mut log,
    );
    tool(&mut s, Tool::Wall);
    record(click(&mut s, 2500.0, 1000.0), &mut log);
    record(click(&mut s, 2500.0, 5000.0), &mut log);
    let bytes = s.export_file("test").unwrap();
    let mut other = state();
    other.import_file(&bytes).unwrap();
    assert_eq!(other.document().variants, s.document().variants);
    assert_eq!(other.document().walls, s.document().walls);
    let mut recovered = state();
    recovered.replay_journal(&log).unwrap();
    assert_eq!(recovered.document().walls, s.document().walls);
}

fn scan_request(mode: crate::protocol::ScanImportMode) -> (crate::protocol::ScanImport, Vec<u8>) {
    let glb = op_scan::io::write_glb(&op_scan::synth::mesh(&op_scan::synth::apartment(0.2)));
    let result = op_scan::analyze(&glb, &mut |_| {}).unwrap();
    (
        crate::protocol::ScanImport {
            params: op_scan::ScanParams {
                ceiling_height: result.ceiling_height,
                ..op_scan::ScanParams::default()
            },
            result,
            answers: Vec::new(),
            mode,
            name: "квартира.glb".into(),
        },
        glb,
    )
}

#[test]
fn scan_import_is_one_step() {
    let mut s = state();
    let (request, glb) = scan_request(crate::protocol::ScanImportMode::Plan);
    let events = s.import_scan(&request, Some(glb.clone()));
    assert!(
        !has_error(&events),
        "{:?}",
        events
            .events
            .iter()
            .find(|e| matches!(e, FromEngine::Error { .. }))
    );
    let doc = s.document();
    assert_eq!(doc.rooms.len(), 2, "две комнаты");
    assert!(doc.walls.len() >= 5, "{}", doc.walls.len());
    assert_eq!(doc.openings.len(), 4);
    assert_eq!(doc.underlays.len(), 1);
    assert_eq!(doc.underlays[0].layer, LayerKind::Scan.id());
    assert!(
        doc.assets
            .keys()
            .any(|k| k.starts_with("scan/") && k.ends_with(".glb"))
    );
    assert!((doc.default_ceiling_height - 2700.0).abs() <= 20.0);
    let mut areas: Vec<f64> = events
        .events
        .iter()
        .find_map(|e| match e {
            FromEngine::DerivedChanged { rooms, .. } => {
                Some(rooms.iter().map(|r| r.area).collect())
            }
            _ => None,
        })
        .unwrap();
    areas.sort_by(f64::total_cmp);
    assert!(
        (areas[0] - 13.52).abs() < 0.25 && (areas[1] - 14.0).abs() < 0.25,
        "{areas:?}"
    );

    // Файл: исходный скан и вид сверху сохраняются.
    let bytes = s.export_file("test").unwrap();
    let mut other = state();
    other.import_file(&bytes).unwrap();
    assert_eq!(other.document().assets, s.document().assets);

    // Одна отмена убирает весь импорт.
    s.handle(ToEngine::Undo);
    assert!(s.document().walls.is_empty() && s.document().assets.is_empty());
}

#[test]
fn scan_underlay_only() {
    let mut s = state();
    let (mut request, _) = scan_request(crate::protocol::ScanImportMode::UnderlayOnly);
    request.params.keep_source = false;
    s.import_scan(&request, None);
    let doc = s.document();
    assert!(doc.walls.is_empty());
    assert_eq!(doc.underlays.len(), 1);
    assert_eq!(doc.assets.len(), 1);
}

#[test]
fn protocol_json_shape() {
    let json = serde_json::to_string(&ToEngine::Pointer {
        phase: PointerPhase::Down,
        x: 1.0,
        y: 2.0,
        shift: false,
        alt: false,
    })
    .unwrap();
    assert_eq!(
        json,
        r#"{"type":"pointer","phase":"down","x":1.0,"y":2.0,"shift":false,"alt":false}"#
    );
}

// ---------------------------------------------------------------- чертежи

fn sheet_texts(sheet: &op_export::Sheet) -> Vec<String> {
    let viewports = sheet.viewports.iter().flat_map(|v| v.items.iter());
    sheet
        .paper
        .iter()
        .chain(viewports)
        .filter_map(|i| match &i.shape {
            op_export::Shape::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

fn request(variant: Option<&str>) -> crate::protocol::DrawingRequest {
    crate::protocol::DrawingRequest {
        variant: variant.map(Into::into),
        level: None,
        date: "11.26".into(),
    }
}

/// Две комнаты с дверью в перегородке и окном в верхней стене; вариант: перегородка
/// демонтирована, новая из ГКЛ поперёк, окно заложено.
fn replanned(s: &mut EngineState) -> String {
    two_rooms(s);
    tool(s, Tool::Door);
    assert!(!has_error(&click(s, 2500.0, 3000.0)));
    tool(s, Tool::Window);
    assert!(!has_error(&click(s, 1750.0, 1000.0)));
    let window = s.document().openings[1].id.clone();
    tool(s, Tool::Select);
    s.handle(ToEngine::CreateVariant {
        name: "Вариант А".into(),
        copy_of: None,
    });
    let v = variant_id(s);
    let wall = partition(s);
    s.handle(ToEngine::SetDemolished {
        ids: vec![wall, window],
        demolished: true,
    });
    s.handle(ToEngine::SetToolOptions {
        options: crate::protocol::ToolOptions {
            wall_thickness: 100.0,
            justification: op_model::Justification::Center,
            material: Some(op_model::WallMaterial::Gypsum),
        },
    });
    tool(s, Tool::Wall);
    click(s, 1000.0, 2000.0);
    click(s, 4000.0, 2000.0);
    s.handle(ToEngine::Cancel);
    s.handle(ToEngine::FindRooms);
    v
}

#[test]
fn survey_sheet_with_title_block_and_schedule() {
    let mut s = state();
    two_rooms(&mut s);
    let mut meta = s.document().meta.clone();
    meta.title = "Квартира".into();
    meta.address = "Москва, ул. Тверская, 1, кв. 5".into();
    meta.designation = "2026-01-АР".into();
    meta.developer = "Иванов".into();
    assert!(!has_error(&s.handle(ToEngine::Command {
        command: Command::SetMeta { meta },
    })));

    let set = s.drawings(&request(None));
    let titles: Vec<&str> = set.sheets.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Обмерный план", "Развёртки стен"]);
    assert_eq!(set.scale, 50, "квартира на A3 — в 1:50");
    assert!(set.auto_scale && set.warnings.is_empty());
    assert_eq!(set.name, "Квартира — Обмер (как есть)");
    let texts = sheet_texts(&set.sheets[0]);
    for expected in [
        "2026-01-АР",
        "Иванов",
        "11.26",
        "Р",
        "Экспликация помещений",
        "1. Помещение 1",
        "Обмерный план (М 1:50)",
    ] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
    // Экспликация: две комнаты 1,38 × 3,88 м (оси через 1500 мм, стены 120 мм).
    let table = &set.worksheets[0].tables[0];
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.rows[0][2].display(), "5,35", "{:?}", table.rows[0]);

    let pdf = set.export(crate::protocol::DrawingFormat::Pdf).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
    // Два листа: SVG и DXF — архивы по листу.
    assert_eq!(set.extension(crate::protocol::DrawingFormat::Svg), "zip");
    let svg = set.export(crate::protocol::DrawingFormat::Svg).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(svg)).unwrap();
    let mut names: Vec<String> = archive
        .file_names()
        .map(|n| n.unwrap().into_owned())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["01 Обмерный план.svg", "02 Развёртки стен.svg"]);
    let mut first = String::new();
    std::io::Read::read_to_string(&mut archive.by_index(0).unwrap(), &mut first).unwrap();
    assert!(first.starts_with("<svg"));
    let dxf = set.export(crate::protocol::DrawingFormat::Dxf).unwrap();
    // DXF — в cp1251, читается байтами.
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(dxf)).unwrap();
    let mut first = Vec::new();
    std::io::Read::read_to_end(&mut archive.by_index(0).unwrap(), &mut first).unwrap();
    assert!(String::from_utf8_lossy(&first).contains("AC1009"));
    let preview = set.preview();
    assert!(preview.sheets[0].svg.contains("<path"));
}

#[test]
fn variant_sheet_set_lists_demolition_and_partitions() {
    let mut s = state();
    let v = replanned(&mut s);
    let set = s.drawings(&request(Some(&v)));
    let titles: Vec<&str> = set.sheets.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Обмерный план",
            "План демонтажа",
            "План монтажа перегородок",
            "План помещений",
            "Развёртки стен"
        ]
    );
    // Все планы в одном масштабе и с одним положением плана.
    let first = &set.sheets[0].viewports[0];
    for sheet in set.sheets.iter().filter(|s| s.title.starts_with("План")) {
        let vp = &sheet.viewports[0];
        assert!((vp.center - first.center).length() < 1e-9);
        assert!((vp.denominator - 50.0).abs() < 1e-9);
    }
    // Номера листов в основной надписи.
    let texts = sheet_texts(&set.sheets[1]);
    assert!(texts.iter().any(|t| t == "2") && texts.iter().any(|t| t == "5"));
    assert!(texts.iter().any(|t| t == "Д1"), "марка на плане: {texts:?}");

    // Демонтаж: перегородка 4 м × 2,7 м (дверь уходит вместе с ней).
    let demolition = &set
        .worksheets
        .iter()
        .find(|w| w.name == "Демонтаж")
        .unwrap()
        .tables[0];
    assert_eq!(demolition.rows.len(), 1, "{:?}", demolition.rows);
    assert_eq!(demolition.rows[0][3], op_export::Cell::number(4.0, 2));
    assert_eq!(demolition.rows[0][5], op_export::Cell::number(10.8, 2));

    // Монтаж: перегородка из ГКЛ 3 м и закладка окна.
    let construction = &set
        .worksheets
        .iter()
        .find(|w| w.name == "Монтаж")
        .unwrap()
        .tables;
    let names: Vec<String> = construction[0]
        .rows
        .iter()
        .map(|r| r[1].display())
        .collect();
    assert_eq!(
        names,
        ["Перегородка: ГКЛ", "Закладка проёма (оконный)"],
        "{names:?}"
    );
    assert_eq!(construction[0].rows[0][3], op_export::Cell::number(3.0, 2));
    let materials: Vec<String> = construction[1]
        .rows
        .iter()
        .map(|r| r[0].display())
        .collect();
    assert_eq!(materials, ["ГКЛ", "Закладка проёмов"]);

    // Пакет для бригады: PDF и XLSX в одном архиве.
    let bundle = set.export(crate::protocol::DrawingFormat::Bundle).unwrap();
    let archive = zip::ZipArchive::new(std::io::Cursor::new(bundle)).unwrap();
    let mut files: Vec<String> = archive
        .file_names()
        .map(|n| n.unwrap().into_owned())
        .collect();
    files.sort_unstable();
    assert_eq!(
        files,
        [
            "Новый проект — Вариант А — ведомости.xlsx",
            "Новый проект — Вариант А.pdf"
        ]
    );
    // Несколько листов SVG — архив.
    assert_eq!(set.extension(crate::protocol::DrawingFormat::Svg), "zip");
    let svgs = set.export(crate::protocol::DrawingFormat::Svg).unwrap();
    let archive = zip::ZipArchive::new(std::io::Cursor::new(svgs)).unwrap();
    assert_eq!(archive.len(), 5);
}

#[test]
fn drawing_scale_follows_plan_size_and_settings() {
    // План 40 × 30 м в 1:50 на A3 не помещается — подбирается 1:200.
    let s = &mut state();
    big_square(s, 40_000.0);
    let set = s.drawings(&request(None));
    assert_eq!(set.scale, 200);
    assert!(set.warnings.is_empty());
    // Без помещений развёрток нет: один лист, SVG — без архива.
    assert_eq!(set.sheets.len(), 1);
    assert_eq!(set.extension(crate::protocol::DrawingFormat::Svg), "svg");
    let svg = set.export(crate::protocol::DrawingFormat::Svg).unwrap();
    assert!(svg.starts_with(b"<svg"));

    // Заданный масштаб не помещается — предупреждение.
    s.handle(ToEngine::Command {
        command: Command::SetDrawing {
            drawing: op_model::DrawingSettings {
                paper: op_model::PaperFormat::A4Portrait,
                scale: Some(50),
            },
        },
    });
    let set = s.drawings(&request(None));
    assert_eq!(set.scale, 50);
    assert!(!set.auto_scale);
    assert_eq!(set.warnings.len(), 1, "{:?}", set.warnings);
    assert!((set.sheets[0].size.x - 210.0).abs() < 1e-9);
}

/// Квадрат стен со стороной `side` мм от начала координат — командами.
fn big_square(s: &mut EngineState, side: f64) {
    let corners = [
        (0.0, 0.0),
        (side, 0.0),
        (side, side * 0.75),
        (0.0, side * 0.75),
    ];
    let mut commands = Vec::new();
    for (i, (x, y)) in corners.iter().enumerate() {
        commands.push(Command::Put {
            element: Element::Node(op_model::Node {
                level: None,
                id: format!("n{i}"),
                variant: None,
                x: *x,
                y: *y,
            }),
            index: None,
        });
    }
    for i in 0..4 {
        commands.push(Command::Put {
            element: Element::Wall(op_model::Wall {
                id: format!("w{i}"),
                layer: LayerKind::Walls.id().into(),
                variant: None,
                a: format!("n{i}"),
                b: format!("n{}", (i + 1) % 4),
                thickness: 200.0,
                justification: op_model::Justification::Center,
                height: None,
                material: None,
            }),
            index: None,
        });
    }
    assert!(!has_error(&s.handle(ToEngine::Command {
        command: Command::Batch { commands },
    })));
}

#[test]
fn wall_elevations_show_openings_and_wall_areas() {
    let mut s = state();
    replanned(&mut s);
    let doc = s.document().clone();
    let (door, window) = (&doc.openings[0], &doc.openings[1]);
    let set = s.drawings(&request(None));
    let sheet = set.sheets.last().unwrap();
    assert_eq!(sheet.title, "Развёртки стен");
    let texts = sheet_texts(sheet);
    for expected in [
        "Развёртки стен (М 1:50)",
        "1. Помещение 1",
        "Стена А",
        "Стена Г",
        "Площади стен",
    ] {
        assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
    }
    // Ширина окна и высота двери — в размерах развёрток.
    let width = crate::geometry::format_length(window.width);
    let height = crate::geometry::format_length(door.height);
    assert!(
        texts.contains(&width) && texts.contains(&height),
        "{texts:?}"
    );

    // Помещение 1: 1,38 × 3,88 м по граням, высота 2,7 м; проёмы — окно и дверь.
    let table = &set
        .worksheets
        .iter()
        .find(|w| w.name == "Стены")
        .unwrap()
        .tables[0];
    let row = &table.rows[0];
    assert_eq!(row[2].display(), "10,52");
    let gross = 10.52 * 2.7;
    let openings = (window.width * window.height + door.width * door.height) / 1e6;
    assert_eq!(row[4].display(), op_export::table::format_number(gross, 2));
    assert_eq!(
        row[5].display(),
        op_export::table::format_number(openings, 2)
    );
    assert_eq!(
        row[6].display(),
        op_export::table::format_number(gross - openings, 2)
    );
    // Дверь в перегородке видна из обоих помещений.
    let room2 = &table.rows[1];
    assert_eq!(
        room2[5].display(),
        op_export::table::format_number(door.width * door.height / 1e6, 2)
    );
}

// ---------------------------------------------------------------- 3D

fn camera_info(events: &EngineEvents) -> crate::protocol::CameraInfo {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::View3dChanged { camera, .. } => Some(*camera),
            _ => None,
        })
        .unwrap()
}

fn enable_3d(s: &mut EngineState) -> EngineEvents {
    s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d {
            enabled: true,
            ..crate::protocol::Options3d::default()
        },
    })
}

#[test]
fn three_d_view_fits_the_model_and_rebuilds_after_edits() {
    let mut s = state();
    two_rooms(&mut s);
    assert!(s.take_scene_3d().is_none(), "в 2D 3D-сцена не строится");
    let events = enable_3d(&mut s);
    assert!(s.three_d());
    // Орбита наведена на середину квартиры (1000–4000 × 1000–5000).
    let info = camera_info(&events);
    assert!((info.target[0] - 2500.0).abs() < 50.0 && (info.target[1] - 3000.0).abs() < 50.0);
    let scene = s.take_scene_3d().unwrap();
    assert!(scene.opaque.triangles() > 50);
    assert!(
        s.take_scene_3d().is_none(),
        "без изменений сцена не пересобирается"
    );
    // Правка документа — новая сцена.
    tool(&mut s, Tool::Door);
    click(&mut s, 2500.0, 3000.0);
    assert!(s.take_scene_3d().is_some());
}

#[test]
fn camera_moves_and_saved_views_are_undoable() {
    let mut s = state();
    two_rooms(&mut s);
    let before = camera_info(&enable_3d(&mut s));
    let after = camera_info(&s.handle(ToEngine::Camera3d {
        action: crate::protocol::CameraAction::Rotate { dx: 100.0, dy: 0.0 },
    }));
    assert!((before.eye[0] - after.eye[0]).abs() > 100.0);

    // Прогулка: WASD двигает глаз на высоте 1,6 м.
    s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d {
            enabled: true,
            mode: crate::protocol::CameraMode::Walk,
            ..crate::protocol::Options3d::default()
        },
    });
    let walk = camera_info(&s.handle(ToEngine::Camera3d {
        action: crate::protocol::CameraAction::Move {
            forward: 1000.0,
            right: 0.0,
            up: 0.0,
        },
    }));
    assert_eq!(walk.mode, crate::protocol::CameraMode::Walk);
    assert!((walk.eye[2] - crate::camera3d::EYE_HEIGHT).abs() < 1e-9);

    // Ракурс сохраняется в проекте одной отменяемой правкой.
    assert!(!has_error(&s.handle(ToEngine::SaveView3d {
        name: "Кухня".into()
    })));
    let saved = s.document().views[0].clone();
    assert_eq!(saved.name, "Кухня");
    assert!(saved.walk);
    s.handle(ToEngine::Camera3d {
        action: crate::protocol::CameraAction::Rotate {
            dx: 300.0,
            dy: 50.0,
        },
    });
    let restored = camera_info(&s.handle(ToEngine::ApplyView3d {
        id: saved.id.clone(),
    }));
    for (a, b) in restored.eye.iter().zip(saved.eye) {
        assert!((a - b).abs() < 1e-6);
    }
    s.handle(ToEngine::Undo);
    assert!(s.document().views.is_empty());
    assert!(has_error(&s.handle(ToEngine::ApplyView3d { id: saved.id })));

    // Ракурсы и привязка к местности — в файле.
    s.handle(ToEngine::Redo);
    let mut site = s.document().site;
    site.north = 30.0;
    s.handle(ToEngine::Command {
        command: Command::SetSite { site },
    });
    let bytes = s.export_file("test").unwrap();
    let mut t = state();
    t.import_file(&bytes).unwrap();
    assert_eq!(t.document().views.len(), 1);
    assert!((t.document().site.north - 30.0).abs() < 1e-9);
}

#[test]
fn three_d_shows_the_variant_and_reports_the_sun() {
    let mut s = state();
    let v = replanned(&mut s);
    s.handle(ToEngine::SetView {
        view: crate::protocol::View::Variant { id: v },
    });
    let events = s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d {
            enabled: true,
            sun: Some(crate::protocol::SunTime {
                day: crate::sun::day_of_year(6, 21),
                hour: 13.0,
            }),
            ..crate::protocol::Options3d::default()
        },
    });
    let sun = events
        .events
        .iter()
        .find_map(|e| match e {
            FromEngine::View3dChanged { sun, .. } => *sun,
            _ => None,
        })
        .unwrap();
    assert!(sun.altitude > 50.0, "{sun:?}");
    let scene = s.take_scene_3d().unwrap();
    assert!(scene.sun.is_some());
    // Новая перегородка из ГКЛ — своего цвета.
    let gypsum = op_render::scene::Color::hex(0xEDE1BE, 1.0).0;
    let same = |a: [f32; 4]| a.iter().zip(gypsum).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(scene.opaque.vertices.iter().any(|v| same(v.color)));
    // Сравнение — только на плане.
    let compare = s.handle(ToEngine::SetView {
        view: crate::protocol::View::Compare {
            left: None,
            right: None,
        },
    });
    assert!(!has_error(&compare));
    s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d {
            enabled: true,
            ..crate::protocol::Options3d::default()
        },
    });
    assert!(!s.three_d());
}

#[test]
fn walk_starts_inside_the_nearest_room() {
    let mut s = state();
    two_rooms(&mut s);
    enable_3d(&mut s);
    let walk = camera_info(&s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d {
            enabled: true,
            mode: crate::protocol::CameraMode::Walk,
            ..crate::protocol::Options3d::default()
        },
    }));
    // Середина квартиры — на перегородке x = 2500; глаз встаёт в подпись помещения.
    let rooms: Vec<(f64, f64)> = s.document().rooms.iter().map(|r| (r.x, r.y)).collect();
    assert!(
        rooms
            .iter()
            .any(|(x, y)| (walk.eye[0] - x).abs() < 1e-6 && (walk.eye[1] - y).abs() < 1e-6),
        "{walk:?} {rooms:?}"
    );
}

// ---------------------------------------------------------------- мебель

fn furniture_tool(s: &mut EngineState, item: &str) {
    assert!(!has_error(
        &s.handle(ToEngine::FurnitureTool { item: item.into() })
    ));
}

#[test]
fn furniture_snaps_to_walls_and_transforms() {
    let mut s = state();
    two_rooms(&mut s);
    furniture_tool(&mut s, "bed-double");
    assert_eq!(s.step(), ToolStep::FurniturePlace);
    // Спинка у верхней стены (внутренняя грань y = 1060): кровать встаёт к ней вплотную.
    click(&mut s, 1700.0, 2150.0);
    let bed = s.document().furniture[0].clone();
    assert_eq!(bed.item, "bed-double");
    assert!((bed.y - 2060.0).abs() < 1e-6, "{}", bed.y);
    assert!(bed.rotation.abs() < 1e-6);
    // У левой стены (грань x = 1060) лицом вправо — поворот 270°.
    furniture_tool(&mut s, "wardrobe");
    click(&mut s, 1420.0, 4000.0);
    let wardrobe = s.document().furniture[1].clone();
    assert!((wardrobe.x - 1360.0).abs() < 1e-6, "{}", wardrobe.x);
    assert!(
        (wardrobe.rotation - 270.0).abs() < 1e-6,
        "{}",
        wardrobe.rotation
    );
    // Посреди комнаты — без привязки, с поворотом инструмента.
    furniture_tool(&mut s, "armchair");
    s.handle(ToEngine::TransformFurniture {
        rotate: 90.0,
        mirror: false,
    });
    click(&mut s, 1750.0, 3500.0);
    let chair = s.document().furniture[2].clone();
    assert!((chair.rotation - 90.0).abs() < 1e-6);

    // Выделение кровати щелчком, поворот и отражение — одна правка каждое.
    tool(&mut s, Tool::Select);
    click(&mut s, 1700.0, 2500.0);
    assert_eq!(s.selection(), std::slice::from_ref(&bed.id));
    s.handle(ToEngine::TransformFurniture {
        rotate: -90.0,
        mirror: true,
    });
    let turned = s.document().furniture_item(&bed.id).unwrap().clone();
    assert!((turned.rotation - 270.0).abs() < 1e-6 && turned.mirrored);
    s.handle(ToEngine::Undo);
    assert!(!s.document().furniture_item(&bed.id).unwrap().mirrored);

    // Перетаскивание: вдоль верхней стены кровать скользит, оставаясь у неё;
    // спинкой к левой стене (грань x = 1060) — поворачивается лицом вправо.
    drag(
        &mut s,
        DVec2::new(1700.0, 2500.0),
        DVec2::new(1500.0, 2500.0),
    );
    let slid = s.document().furniture_item(&bed.id).unwrap().clone();
    assert!(
        (slid.x - 1500.0).abs() < 1e-6 && (slid.y - 2060.0).abs() < 1e-6,
        "{slid:?}"
    );
    drag(
        &mut s,
        DVec2::new(1500.0, 2500.0),
        DVec2::new(2100.0, 3440.0),
    );
    let moved = s.document().furniture_item(&bed.id).unwrap().clone();
    assert!(
        (moved.x - 2060.0).abs() < 1e-6 && (moved.rotation - 270.0).abs() < 1e-6,
        "{moved:?}"
    );

    // Удаление.
    s.handle(ToEngine::Select {
        ids: vec![chair.id.clone()],
    });
    s.handle(ToEngine::DeleteSelection);
    assert!(s.document().furniture_item(&chair.id).is_none());
}

#[test]
fn own_glb_model_is_stored_drawn_and_removed() {
    let cube = op_scan::io::Mesh {
        positions: vec![
            glam::DVec3::new(0.0, 0.0, 0.0),
            glam::DVec3::new(0.8, 0.0, 0.0),
            glam::DVec3::new(0.8, 0.0, 0.5),
            glam::DVec3::new(0.0, 0.0, 0.5),
            glam::DVec3::new(0.4, 1.2, 0.25),
        ],
        triangles: vec![
            [0, 1, 4],
            [1, 2, 4],
            [2, 3, 4],
            [3, 0, 4],
            [0, 2, 1],
            [0, 3, 2],
        ],
    };
    let bytes = op_scan::io::write_glb(&cube);
    let mut s = state();
    two_rooms(&mut s);
    s.import_furniture(bytes, "Кресло-мешок.glb").unwrap();
    let f = s.document().furniture[0].clone();
    assert_eq!(f.item, crate::furniture::CUSTOM);
    assert_eq!(f.name, "Кресло-мешок");
    assert_eq!([f.width, f.depth, f.height], [800.0, 500.0, 1200.0]);
    let asset = f.model.clone().unwrap();
    assert!(s.document().assets.contains_key(&asset));
    // В 3D — треугольники модели.
    enable_3d(&mut s);
    let scene = s.take_scene_3d().unwrap();
    let tall = scene
        .opaque
        .vertices
        .iter()
        .any(|v| (f64::from(v.pos[2]) - 1200.0).abs() < 1.0);
    assert!(tall, "вершина модели на высоте 1,2 м");
    // Модель переживает файл; при удалении предмета уходит и файл модели.
    let file = s.export_file("test").unwrap();
    let mut t = state();
    t.import_file(&file).unwrap();
    assert!(t.models.contains_key(&asset));
    s.handle(ToEngine::Select { ids: vec![f.id] });
    s.handle(ToEngine::DeleteSelection);
    assert!(!s.document().assets.contains_key(&asset));
    assert!(s.import_furniture(b"not a glb".to_vec(), "x").is_err());
}

#[test]
fn furniture_belongs_to_variants_and_drawings() {
    let mut s = state();
    two_rooms(&mut s);
    // Мебель обмера — диван; в варианте его убирают и ставят кровать.
    furniture_tool(&mut s, "sofa");
    click(&mut s, 1700.0, 4400.0);
    let sofa = s.document().furniture[0].id.clone();
    tool(&mut s, Tool::Select);
    s.handle(ToEngine::CreateVariant {
        name: "Вариант А".into(),
        copy_of: None,
    });
    let v = variant_id(&s);
    s.handle(ToEngine::SetDemolished {
        ids: vec![sofa.clone()],
        demolished: true,
    });
    furniture_tool(&mut s, "bed-single");
    click(&mut s, 3200.0, 2000.0);
    let bed = s.document().furniture.last().unwrap().clone();
    assert_eq!(bed.variant.as_deref(), Some(v.as_str()));
    assert!(
        s.doc().furniture_item(&sofa).is_none(),
        "диван убран в варианте"
    );
    assert!(
        s.document().furniture_item(&sofa).is_some(),
        "в обмере диван остался"
    );

    // Комплект варианта: план расстановки и спецификация.
    let set = s.drawings(&request(Some(&v)));
    let sheet = set
        .sheets
        .iter()
        .find(|s| s.title == "План расстановки мебели")
        .unwrap();
    let texts = sheet_texts(sheet);
    assert!(
        texts
            .iter()
            .any(|t| t == "Спецификация мебели и оборудования")
    );
    assert!(texts.iter().any(|t| t == "Кровать односпальная"));
    assert!(!texts.iter().any(|t| t == "Диван"), "{texts:?}");
    let table = &set
        .worksheets
        .iter()
        .find(|w| w.name == "Мебель")
        .unwrap()
        .tables[0];
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0][2].display(), "900 × 2000 × 450");

    // Копия варианта переносит мебель, удаление варианта убирает её.
    s.handle(ToEngine::CreateVariant {
        name: "Копия".into(),
        copy_of: Some(v.clone()),
    });
    assert_eq!(s.document().furniture.len(), 3);
    let copy = variant_id(&s);
    s.handle(ToEngine::DeleteVariant { id: copy });
    assert_eq!(s.document().furniture.len(), 2);
}

// ---------------------------------------------------------------- электрика

fn device_tool(s: &mut EngineState, kind: op_model::DeviceKind) {
    assert!(!has_error(&s.handle(ToEngine::DeviceTool { kind })));
}

fn electrical(events: &EngineEvents) -> crate::protocol::ElectricalReport {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::Electrical { report } => Some(report.clone()),
            _ => None,
        })
        .unwrap()
}

/// Две комнаты: розетки, плита 380 В, светильники, двухклавишный выключатель и щит.
fn wired(s: &mut EngineState) -> (String, String, String) {
    use op_model::DeviceKind as K;
    two_rooms(s);
    device_tool(s, K::Panel);
    click(s, 1100.0, 4500.0);
    device_tool(s, K::Socket);
    click(s, 1700.0, 1100.0);
    click(s, 3200.0, 1100.0);
    device_tool(s, K::SocketPower);
    click(s, 3900.0, 3000.0);
    device_tool(s, K::CeilingLight);
    click(s, 1750.0, 3000.0);
    let a = s.document().devices.last().unwrap().id.clone();
    click(s, 3250.0, 3000.0);
    let b = s.document().devices.last().unwrap().id.clone();
    device_tool(s, K::Switch2);
    click(s, 2440.0, 4600.0);
    let switch = s.document().devices.last().unwrap().clone();
    tool(s, Tool::Select);
    let mut linked = switch.clone();
    linked.keys = vec![vec![a.clone()], vec![b.clone()]];
    assert!(!has_error(&s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::Device(linked),
            index: None,
        },
    })));
    (switch.id, a, b)
}

#[test]
fn devices_sit_on_wall_faces_and_ceilings() {
    use op_model::DeviceKind as K;
    let mut s = state();
    two_rooms(&mut s);
    // Розетка у верхней стены (грань y = 1060): центр на половину обозначения от грани.
    device_tool(&mut s, K::Socket);
    assert_eq!(s.step(), ToolStep::DevicePlace);
    click(&mut s, 1700.0, 1150.0);
    let socket = s.document().devices[0].clone();
    assert!(
        (socket.y - (1060.0 + crate::electric::SYMBOL / 2.0)).abs() < 1e-6,
        "{socket:?}"
    );
    assert!(socket.rotation.abs() < 1e-6);
    assert!((socket.height - 300.0).abs() < 1e-9 && (socket.power - 300.0).abs() < 1e-9);
    // Светильник — где щёлкнули, на высоте потолка, с потоком по умолчанию.
    device_tool(&mut s, K::CeilingLight);
    click(&mut s, 1750.0, 3000.0);
    let light = s.document().devices[1].clone();
    assert!((light.x - 1750.0).abs() < 1e-6 && (light.height - 2700.0).abs() < 1e-9);
    assert!(light.flux > 0.0);
    // Выключатель с двумя клавишами.
    device_tool(&mut s, K::Switch2);
    click(&mut s, 1100.0, 4000.0);
    let switch = s.document().devices[2].clone();
    assert_eq!(switch.keys.len(), 2);
    assert!(
        (switch.rotation - 270.0).abs() < 1e-6,
        "лицом вправо от левой стены"
    );
}

#[test]
fn circuits_are_allocated_and_calculated() {
    let mut s = state();
    wired(&mut s);
    let events = s.handle(ToEngine::AutoCircuits);
    assert!(!has_error(&events));
    let doc = s.document();
    let purposes: Vec<&str> = doc.circuits.iter().map(|c| c.purpose.as_str()).collect();
    assert_eq!(purposes[0], "Освещение");
    assert!(purposes.contains(&"Розетка 380 В"), "{purposes:?}");
    assert!(
        purposes.iter().any(|p| p.starts_with("Розетки")),
        "{purposes:?}"
    );
    let power = doc
        .circuits
        .iter()
        .find(|c| c.purpose == "Розетка 380 В")
        .unwrap();
    assert_eq!(power.phases, 3);
    // Щит, выключатель — без группы; потребители распределены.
    let report = electrical(&events);
    assert_eq!(report.unassigned, 0);
    let lighting = &report.circuits[0];
    assert!((lighting.installed - 80.0).abs() < 1e-9);
    assert!(
        lighting.length > 1.0 && lighting.warnings.is_empty(),
        "{lighting:?}"
    );
    // Трёхфазный ввод: в щите вводной автомат 3P.
    assert!(report.panel.three_phase);
    assert!(
        report
            .panel
            .items
            .iter()
            .any(|i| i.name.starts_with("Вводной автомат 3P"))
    );
    assert!(report.panel.enclosure >= report.panel.modules);
    // Освещённость: по светильнику 3500 лм на комнату ≈ 5 м².
    assert_eq!(report.rooms.len(), 2);
    assert!(
        report
            .rooms
            .iter()
            .all(|r| r.lux > 100.0 && (r.norm - 150.0).abs() < 1e-9)
    );

    // Перегруз: розетка на 7 кВт в группе на 16 А — предупреждение.
    let socket = s
        .document()
        .devices
        .iter()
        .find(|d| d.kind == op_model::DeviceKind::Socket)
        .unwrap()
        .clone();
    let events = s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::Device(op_model::Device {
                power: 7000.0,
                ..socket.clone()
            }),
            index: None,
        },
    });
    let report = electrical(&events);
    let group = report
        .circuits
        .iter()
        .find(|c| Some(&c.id) == socket.circuit.as_ref())
        .unwrap();
    assert!(
        group
            .warnings
            .iter()
            .any(|w| w.contains("больше тока автомата")),
        "{group:?}"
    );
}

#[test]
fn deleting_a_light_unlinks_switches_and_scenes() {
    let mut s = state();
    let (switch, a, b) = wired(&mut s);
    s.handle(ToEngine::Command {
        command: Command::SetScenes {
            scenes: vec![op_model::LightScene {
                id: "scene".into(),
                name: "Вечер".into(),
                lights: vec![a.clone(), b.clone()],
            }],
        },
    });
    s.handle(ToEngine::Select {
        ids: vec![a.clone()],
    });
    s.handle(ToEngine::DeleteSelection);
    let doc = s.document();
    assert!(doc.device(&a).is_none());
    assert_eq!(
        doc.device(&switch).unwrap().keys,
        vec![vec![], vec![b.clone()]]
    );
    assert_eq!(doc.scenes[0].lights, vec![b]);
    // Одна отмена возвращает всё.
    s.handle(ToEngine::Undo);
    assert_eq!(s.document().device(&switch).unwrap().keys[0], vec![a]);
}

#[test]
fn electrical_sheets_and_3d() {
    let mut s = state();
    wired(&mut s);
    s.handle(ToEngine::AutoCircuits);
    let set = s.drawings(&request(None));
    let titles: Vec<&str> = set.sheets.iter().map(|s| s.title.as_str()).collect();
    for expected in [
        "План розеток и выводов",
        "План освещения",
        "План выключателей",
        "План электропроводки",
    ] {
        assert!(titles.contains(&expected), "{titles:?}");
    }
    let switches = set
        .sheets
        .iter()
        .find(|s| s.title == "План выключателей")
        .unwrap();
    let texts = sheet_texts(switches);
    assert!(texts.iter().any(|t| t == "Управление светом"));
    assert!(texts.iter().any(|t| t == "С1"), "{texts:?}");
    let wiring = set
        .sheets
        .iter()
        .find(|s| s.title == "План электропроводки")
        .unwrap();
    let texts = sheet_texts(wiring);
    assert!(texts.iter().any(|t| t == "Состав щита") && texts.iter().any(|t| t == "Гр.1"));
    let book = set
        .worksheets
        .iter()
        .find(|w| w.name == "Электрика")
        .unwrap();
    assert!(book.tables.len() >= 4);
    // В 3D — светильники под потолком.
    enable_3d(&mut s);
    let scene = s.take_scene_3d().unwrap();
    let glow = op_render::scene::Color::hex(0xFFF1C2, 1.0).0;
    let same = |a: [f32; 4]| a.iter().zip(glow).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(
        scene
            .opaque
            .vertices
            .iter()
            .any(|v| same(v.color) && (f64::from(v.pos[2]) - 2700.0).abs() < 1.0)
    );
}

#[test]
fn variant_copy_keeps_switch_links() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    let v = variant_id(&s);
    device_tool(&mut s, op_model::DeviceKind::CeilingLight);
    click(&mut s, 1750.0, 3000.0);
    let light = s.document().devices[0].id.clone();
    device_tool(&mut s, op_model::DeviceKind::Switch1);
    click(&mut s, 1100.0, 4000.0);
    let mut switch = s.document().devices[1].clone();
    switch.keys = vec![vec![light.clone()]];
    s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::Device(switch),
            index: None,
        },
    });
    s.handle(ToEngine::CreateVariant {
        name: "Б".into(),
        copy_of: Some(v),
    });
    let copy = variant_id(&s);
    let doc = s.document();
    let copied: Vec<&op_model::Device> = doc
        .devices
        .iter()
        .filter(|d| d.variant.as_deref() == Some(copy.as_str()))
        .collect();
    assert_eq!(copied.len(), 2);
    let new_light = copied.iter().find(|d| d.kind.is_light()).unwrap();
    let new_switch = copied.iter().find(|d| !d.kind.is_light()).unwrap();
    assert_ne!(new_light.id, light);
    assert_eq!(new_switch.keys, vec![vec![new_light.id.clone()]]);
}

// ---------------------------------------------------------------- сантехника

fn plumbing(events: &EngineEvents) -> crate::protocol::PlumbingReport {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::Plumbing { report } => Some(report.clone()),
            _ => None,
        })
        .unwrap()
}

fn plumbing_tool(s: &mut EngineState, item: &str) {
    assert!(!has_error(
        &s.handle(ToEngine::PlumbingTool { item: item.into() })
    ));
}

/// Две комнаты; в левой у верхней стены — раковина и унитаз, внизу — стояки.
fn bathroom(s: &mut EngineState) {
    two_rooms(s);
    furniture_tool(s, "washbasin");
    click(s, 1500.0, 1300.0);
    furniture_tool(s, "toilet");
    click(s, 2150.0, 1400.0);
    plumbing_tool(s, "riser-drain");
    click(s, 2300.0, 2000.0);
    plumbing_tool(s, "riser-cold");
    click(s, 2300.0, 2250.0);
    plumbing_tool(s, "riser-hot");
    click(s, 2150.0, 2250.0);
    tool(s, Tool::Select);
}

#[test]
fn fixtures_get_outlets_and_pipes_route_to_risers() {
    use op_model::{PipeSystem, PlumbingKind};
    let mut s = state();
    bathroom(&mut s);
    assert_eq!(s.document().plumbing.len(), 3);
    let events = s.handle(ToEngine::FixtureOutlets);
    assert!(!has_error(&events));
    let outlets: Vec<_> = s
        .document()
        .plumbing
        .iter()
        .filter(|p| p.kind == PlumbingKind::Outlet)
        .cloned()
        .collect();
    // Раковина: ГВС, ХВС, канализация; унитаз: ХВС и канализация Ø110.
    assert_eq!(outlets.len(), 5);
    assert!(
        outlets.iter().all(|o| (o.y - 1060.0).abs() < 1e-6),
        "{outlets:?}"
    );
    assert!(outlets.iter().all(|o| o.fixture.is_some()));
    assert!(
        outlets
            .iter()
            .any(|o| o.system == PipeSystem::Drain && (o.diameter - 110.0).abs() < 1e-9)
    );
    assert_eq!(plumbing(&events).unconnected, 5);
    // Повторно выводы не ставятся.
    assert!(has_error(&s.handle(ToEngine::FixtureOutlets)));

    let events = s.handle(ToEngine::RoutePipes);
    assert!(!has_error(&events));
    let report = plumbing(&events);
    assert_eq!(s.document().pipes.len(), 5);
    assert_eq!(report.unconnected, 0);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    // Канализация раковины — с нормальным уклоном 3 %, без замечаний.
    let drain = report
        .pipes
        .iter()
        .find(|p| p.system == PipeSystem::Drain && (p.diameter - 50.0).abs() < 1e-9)
        .unwrap();
    assert!((drain.slope.unwrap() - 0.03).abs() < 1e-3, "{drain:?}");
    assert!(drain.warnings.is_empty(), "{drain:?}");
    let names: Vec<&str> = report.bill.iter().map(|r| r.name.as_str()).collect();
    for expected in [
        "Труба PP-R PN20 Ø20",
        "Труба канализационная ПП Ø110",
        "Водорозетка Ø20×1/2″",
        "Тройник канализационный 110×50",
    ] {
        assert!(names.contains(&expected), "{names:?}");
    }
    // Новая прокладка заменяет автоматические трубы, а не добавляет.
    s.handle(ToEngine::RoutePipes);
    assert_eq!(s.document().pipes.len(), 5);
    // Отмена — одним шагом.
    s.handle(ToEngine::Undo);
    assert_eq!(s.document().pipes.len(), 5);
    s.handle(ToEngine::Undo);
    assert!(s.document().pipes.is_empty());
}

#[test]
fn drain_checks_and_revision_access() {
    use op_model::{PipeSystem, PipeVertex};
    let mut s = state();
    bathroom(&mut s);
    s.handle(ToEngine::FixtureOutlets);
    // Врезка стояка выше слива унитаза: не дотянуться.
    let mut riser = s
        .document()
        .plumbing
        .iter()
        .find(|p| p.kind == op_model::PlumbingKind::Riser && p.system == PipeSystem::Drain)
        .unwrap()
        .clone();
    riser.height = 450.0;
    s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::PlumbingPoint(riser.clone()),
            index: None,
        },
    });
    let report = plumbing(&s.handle(ToEngine::RoutePipes));
    assert!(
        report
            .pipes
            .iter()
            .any(|p| p.warnings.iter().any(|w| w.contains("ниже врезки"))),
        "{:?}",
        report.pipes
    );
    // Ручная труба почти без уклона.
    let flat = op_model::Pipe {
        level: None,
        id: "flat".into(),
        layer: LayerKind::Plumbing.id().into(),
        variant: None,
        system: PipeSystem::Drain,
        diameter: 50.0,
        points: vec![
            PipeVertex {
                x: 1200.0,
                y: 4000.0,
                z: 400.0,
            },
            PipeVertex {
                x: 2200.0,
                y: 4000.0,
                z: 395.0,
            },
        ],
        auto: false,
    };
    let report = plumbing(&s.handle(ToEngine::Command {
        command: Command::Put {
            element: Element::Pipe(flat),
            index: None,
        },
    }));
    let pipe = report.pipes.iter().find(|p| p.id == "flat").unwrap();
    assert!(
        pipe.warnings
            .iter()
            .any(|w| w.contains("меньше наименьшего")),
        "{pipe:?}"
    );
    // Шкаф над стояком закрывает ревизию.
    furniture_tool(&mut s, "wardrobe");
    let events = click(&mut s, riser.x, riser.y + 200.0);
    let report = plumbing(&events);
    assert!(
        report.warnings.iter().any(|w| w.contains("ревизи")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn pipe_tool_snaps_to_points_and_finishes_with_enter() {
    use op_model::PipeSystem;
    let mut s = state();
    bathroom(&mut s);
    s.handle(ToEngine::PipeTool {
        system: PipeSystem::Cold,
        diameter: 25.0,
    });
    assert_eq!(s.step(), ToolStep::PipeStart);
    // Первая вершина — рядом со стояком ХВС: встаёт в его центр.
    click(&mut s, 2310.0, 2240.0);
    assert_eq!(s.step(), ToolStep::PipeNext);
    click(&mut s, 2300.0, 3500.0);
    click(&mut s, 1500.0, 3500.0);
    s.handle(ToEngine::Finish);
    assert_eq!(s.step(), ToolStep::PipeStart);
    let pipe = s.document().pipes[0].clone();
    assert!(!pipe.auto && pipe.points.len() == 3);
    assert!((pipe.points[0].x - 2300.0).abs() < 1e-6 && (pipe.points[0].y - 2250.0).abs() < 1e-6);
    // Вода — на отметке стояка.
    assert!(pipe.points.iter().all(|v| (v.z - 300.0).abs() < 1e-6));
    // Одна вершина — трубы нет.
    click(&mut s, 3000.0, 3000.0);
    s.handle(ToEngine::Finish);
    assert_eq!(s.document().pipes.len(), 1);
    // Труба выделяется щелчком и переносится.
    tool(&mut s, Tool::Select);
    click(&mut s, 1900.0, 3500.0);
    assert_eq!(s.selection(), std::slice::from_ref(&pipe.id));
    drag(
        &mut s,
        DVec2::new(1900.0, 3500.0),
        DVec2::new(1900.0, 3600.0),
    );
    let moved = &s.document().pipes[0].points;
    assert!((moved[2].y - 3600.0).abs() < 1.0, "{moved:?}");
}

#[test]
fn plumbing_sheets_3d_and_variant_copy() {
    let mut s = state();
    bathroom(&mut s);
    s.handle(ToEngine::FixtureOutlets);
    s.handle(ToEngine::RoutePipes);
    let set = s.drawings(&request(None));
    let titles: Vec<&str> = set.sheets.iter().map(|s| s.title.as_str()).collect();
    assert!(
        titles.contains(&"План водоснабжения") && titles.contains(&"План канализации"),
        "{titles:?}"
    );
    let sewer = set
        .sheets
        .iter()
        .find(|s| s.title == "План канализации")
        .unwrap();
    let texts = sheet_texts(sewer);
    assert!(texts.iter().any(|t| t == "Трубы канализации"));
    assert!(texts.iter().any(|t| t == "К1"), "{texts:?}");
    assert!(set.worksheets.iter().any(|w| w.name == "Сантехника"));
    // В 3D — трубы цветом системы.
    enable_3d(&mut s);
    let scene = s.take_scene_3d().unwrap();
    let cold = op_render::scene::Color::hex(0x5B8DEF, 1.0).0;
    let same = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(scene.opaque.vertices.iter().any(|v| same(v.color, cold)));
    s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d::default(),
    });
    // Копия варианта: выводы ссылаются на приборы копии.
    s.handle(ToEngine::CreateVariant {
        name: "А".into(),
        copy_of: None,
    });
    let a = variant_id(&s);
    s.handle(ToEngine::FurnitureTool {
        item: "washer".into(),
    });
    click(&mut s, 3500.0, 1400.0);
    s.handle(ToEngine::FixtureOutlets);
    s.handle(ToEngine::CreateVariant {
        name: "Б".into(),
        copy_of: Some(a.clone()),
    });
    let b = variant_id(&s);
    let doc = s.document();
    let washer = doc
        .furniture
        .iter()
        .find(|f| f.variant.as_deref() == Some(b.as_str()))
        .unwrap();
    let outlets: Vec<_> = doc
        .plumbing
        .iter()
        .filter(|p| p.variant.as_deref() == Some(b.as_str()))
        .collect();
    assert_eq!(outlets.len(), 2);
    assert!(
        outlets
            .iter()
            .all(|p| p.fixture.as_deref() == Some(washer.id.as_str()))
    );
}

// ---------------------------------------------------------------- отделка

fn finishes(events: &EngineEvents) -> crate::protocol::FinishReport {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::Finishes { report } => Some(report.clone()),
            _ => None,
        })
        .unwrap()
}

fn left_room(s: &EngineState) -> String {
    s.document()
        .rooms
        .iter()
        .find(|r| r.x < 2500.0)
        .unwrap()
        .id
        .clone()
}

fn finish(id: &str, room: &str, surface: op_model::Surface, material: &str) -> op_model::Finish {
    op_model::Finish {
        id: id.into(),
        room: room.into(),
        surface,
        face: None,
        material: material.into(),
        bottom: 0.0,
        top: None,
        layout: op_model::TileLayout::default(),
    }
}

fn set_finishes(s: &mut EngineState, list: Vec<op_model::Finish>) -> EngineEvents {
    s.handle(ToEngine::Command {
        command: Command::SetFinishes { finishes: list },
    })
}

/// Левая комната: пол — керамогранит, стены — краска, стена А — плитка до 1500,
/// потолок — натяжной.
fn finished(s: &mut EngineState) -> EngineEvents {
    use op_model::Surface;
    two_rooms(s);
    let room = left_room(s);
    let mut apron = finish("apron", &room, Surface::Walls, "tile-300x600");
    apron.face = Some(0);
    apron.top = Some(1500.0);
    set_finishes(
        s,
        vec![
            finish("floor", &room, Surface::Floor, "porcelain-600x600"),
            finish("paint", &room, Surface::Walls, "paint-white"),
            apron,
            finish("ceiling", &room, Surface::Ceiling, "stretch-matte"),
        ],
    )
}

#[test]
fn finishes_compute_areas_and_consumption() {
    let mut s = state();
    let events = finished(&mut s);
    assert!(!has_error(&events));
    let report = finishes(&events);
    // Левая комната 1380 × 3880 мм (грани стен толщиной 120).
    let room = report.rooms.iter().find(|r| r.id == left_room(&s)).unwrap();
    assert!((room.floor - 5.35).abs() < 0.01, "{room:?}");
    assert!((room.perimeter - 10.52).abs() < 0.01);
    assert_eq!(room.faces.len(), 4);
    assert_eq!(room.faces[0].letter, "А");
    assert!((room.faces[0].length - 1380.0).abs() < 1.0);
    let by = |id: &str| report.finishes.iter().find(|f| f.id == id).unwrap().clone();
    // Плитка до 1500 на стене А закрывает краску в своей полосе.
    let apron = by("apron");
    assert!((apron.area - 2.07).abs() < 0.01, "{apron:?}");
    let paint = by("paint");
    assert!(
        (paint.area - (room.walls_net - 2.07)).abs() < 0.02,
        "{paint:?}"
    );
    assert!(
        paint
            .quantities
            .iter()
            .any(|q| q.name == "Краска" && q.packs >= 1)
    );
    let floor = by("floor");
    assert!(floor.whole > 0 && floor.cut > 0);
    let names: Vec<&str> = floor.quantities.iter().map(|q| q.name.as_str()).collect();
    assert_eq!(names, ["Плитка", "Клей", "Затирка"]);
    let ceiling = by("ceiling");
    assert!((ceiling.area - room.ceiling).abs() < 0.01);
    assert_eq!(report.totals.len(), 4);
}

#[test]
fn layout_start_is_optimized_and_undoable() {
    use op_model::Surface;
    let mut s = state();
    two_rooms(&mut s);
    let room = left_room(&s);
    let mut floor = finish("floor", &room, Surface::Floor, "porcelain-600x600");
    // Старт в 50 мм от стены: узкие полосы у левой стены.
    floor.layout.origin = [50.0, 0.0];
    let before = finishes(&set_finishes(&mut s, vec![floor]));
    let info = &before.finishes[0];
    assert!(info.narrow > 0, "{info:?}");
    assert!(info.warnings.iter().any(|w| w.contains("узкие")));
    let after = finishes(&s.handle(ToEngine::OptimizeLayout { id: "floor".into() }));
    assert_eq!(after.finishes[0].narrow, 0, "{:?}", after.finishes[0]);
    assert!(after.finishes[0].cut <= info.cut);
    s.handle(ToEngine::Undo);
    assert!((s.document().finishes[0].layout.origin[0] - 50.0).abs() < 1e-9);
    // Для краски старт не подбирается.
    set_finishes(
        &mut s,
        vec![finish("p", &room, Surface::Walls, "paint-white")],
    );
    assert!(has_error(
        &s.handle(ToEngine::OptimizeLayout { id: "p".into() })
    ));
}

#[test]
fn finish_sheets_elevations_and_cleanup() {
    let mut s = state();
    finished(&mut s);
    let set = s.drawings(&request(None));
    let titles: Vec<&str> = set.sheets.iter().map(|s| s.title.as_str()).collect();
    assert!(
        titles.contains(&"План полов") && titles.contains(&"План потолков"),
        "{titles:?}"
    );
    let floors = set.sheets.iter().find(|s| s.title == "План полов").unwrap();
    let texts = sheet_texts(floors);
    assert!(texts.iter().any(|t| t == "Ведомость отделки полов"));
    assert!(texts.iter().any(|t| t == "Расход отделочных материалов"));
    let elevations = set
        .sheets
        .iter()
        .find(|s| s.title == "Развёртки стен")
        .unwrap();
    let texts = sheet_texts(elevations);
    assert!(
        texts.iter().any(|t| t == "Плитка настенная 300×600"),
        "{texts:?}"
    );
    assert!(set.worksheets.iter().any(|w| w.name == "Отделка"));
    // Плитка с узкой подрезкой — предупреждение на развёртке.
    let mut list = s.document().finishes.clone();
    list[2].layout.origin = [30.0, 0.0];
    set_finishes(&mut s, list);
    let set = s.drawings(&request(None));
    let elevations = set
        .sheets
        .iter()
        .find(|s| s.title == "Развёртки стен")
        .unwrap();
    assert!(
        sheet_texts(elevations)
            .iter()
            .any(|t| t.starts_with("узкие подрезки"))
    );
    // В 3D пол — цветом керамогранита.
    enable_3d(&mut s);
    let scene = s.take_scene_3d().unwrap();
    let floor = op_render::scene::Color::hex(0xB9B4AA, 1.0).0;
    let same = |a: [f32; 4]| a.iter().zip(floor).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(scene.opaque.vertices.iter().any(|v| same(v.color)));
    s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d::default(),
    });
    // Раскладка пола на плане — по запросу UI.
    s.handle(ToEngine::ShowFinishes { show: true });
    let (document, _) = s.take_scenes();
    assert!(document.is_some());
    // Удалённое помещение уносит свою отделку.
    let room = left_room(&s);
    s.handle(ToEngine::Select { ids: vec![room] });
    s.handle(ToEngine::DeleteSelection);
    assert!(s.document().finishes.is_empty());
}

// ---------------------------------------------------------------- смета

fn estimate(events: &EngineEvents) -> crate::protocol::EstimateReport {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::Estimate { report } => Some(report.clone()),
            _ => None,
        })
        .unwrap()
}

fn line<'a>(
    report: &'a crate::protocol::EstimateReport,
    code: &str,
    room: Option<&str>,
) -> Option<&'a crate::protocol::EstimateLine> {
    report
        .lines
        .iter()
        .find(|l| l.code == code && l.room.as_deref() == room)
}

#[test]
fn estimate_takes_quantities_from_the_model() {
    let mut s = state();
    finished(&mut s);
    let room = left_room(&s);
    device_tool(&mut s, op_model::DeviceKind::Socket);
    click(&mut s, 1700.0, 1150.0);
    click(&mut s, 3000.0, 1150.0);
    tool(&mut s, Tool::Select);
    assert!(estimate(&s.snapshot()).lines.is_empty());
    let report = estimate(&s.handle(ToEngine::DefaultPrices));
    // Повторно типовой прайс не добавляется.
    assert!(has_error(&s.handle(ToEngine::DefaultPrices)));
    // Плитка на полу левой комнаты — по площади отделки.
    let floor = line(&report, "О-01", Some(&room)).unwrap();
    assert!((floor.quantity - 5.35).abs() < 0.01, "{floor:?}");
    assert!((floor.cost - floor.quantity * 1900.0).abs() < 0.01);
    assert_eq!(floor.room_name, "Помещение 1");
    // Розетки — по помещениям, где стоят.
    let sockets: f64 = report
        .lines
        .iter()
        .filter(|l| l.code == "Э-01")
        .map(|l| l.quantity)
        .sum();
    assert!((sockets - 2.0).abs() < 1e-9);
    // Грунтовка — норма 0,15 л/м² и запас 10 % от площади стен.
    let walls = finishes(&s.snapshot())
        .rooms
        .iter()
        .find(|r| r.id == room)
        .unwrap()
        .walls_net;
    let primer = line(&report, "О-08", Some(&room)).unwrap();
    assert!((primer.quantity - (walls * 0.15 * 1.1 * 100.0).round() / 100.0).abs() < 1e-9);
    // Клей и затирка — из расчёта отделки, в списке закупок.
    for code in ["О-10", "О-11", "О-09"] {
        assert!(report.purchases.iter().any(|p| p.code == code), "{code}");
    }
    assert!(report.purchases.iter().all(|p| p.code != "О-01"));
    // Итоги сходятся.
    let sum: f64 = report.lines.iter().map(|l| l.cost).sum();
    assert!((report.total - sum).abs() < 0.05);
    assert!((report.total - (report.works + report.materials)).abs() < 0.01);
    assert!(report.sections.iter().any(|g| g.name == "Отделка"));
    assert!(report.rooms.iter().any(|g| g.name == "Помещение 1"));
}

#[test]
fn manual_quantities_override_and_undo() {
    let mut s = state();
    finished(&mut s);
    let room = left_room(&s);
    s.handle(ToEngine::DefaultPrices);
    let events = s.handle(ToEngine::Command {
        command: Command::SetOverrides {
            overrides: vec![op_model::QuantityOverride {
                item: "price-О-01".into(),
                room: Some(room.clone()),
                quantity: 6.0,
            }],
        },
    });
    let l = line(&estimate(&events), "О-01", Some(&room))
        .unwrap()
        .clone();
    assert!(l.overridden && (l.quantity - 6.0).abs() < 1e-9);
    assert!((l.cost - 11_400.0).abs() < 1e-9);
    // Позиция с ручным объёмом — строка есть сразу, количество задаёт пользователь.
    let mut prices = s.document().prices.clone();
    prices.push(op_model::PriceItem {
        id: "own".into(),
        code: "Х-01".into(),
        section: "Прочее".into(),
        name: "Доставка".into(),
        kind: op_model::PriceKind::Work,
        unit: "шт.".into(),
        price: 3000.0,
        measure: op_model::Measure::Manual,
        rate: 1.0,
        reserve: 0.0,
        history: Vec::new(),
    });
    let report = estimate(&s.handle(ToEngine::Command {
        command: Command::SetPrices { prices },
    }));
    let own = line(&report, "Х-01", None).unwrap();
    assert!((own.quantity).abs() < 1e-9 && own.room_name == crate::estimate::NO_ROOM);
    s.handle(ToEngine::Undo);
    s.handle(ToEngine::Undo);
    let report = estimate(&s.snapshot());
    assert!(!line(&report, "О-01", Some(&room)).unwrap().overridden);
}

#[test]
fn variants_are_priced_for_comparison() {
    let mut s = state();
    replanned(&mut s);
    s.handle(ToEngine::DefaultPrices);
    let views = summaries(&s.snapshot());
    let survey = views.iter().find(|v| v.variant.is_none()).unwrap();
    let variant = views.iter().find(|v| v.variant.is_some()).unwrap();
    // В варианте — демонтаж, перегородка из ГКЛ и закладка: дороже обмера.
    assert!(
        variant.cost > survey.cost,
        "{} > {}",
        variant.cost,
        survey.cost
    );
    let report = estimate(&s.snapshot());
    assert!(line(&report, "Д-01", None).is_some());
    assert!(line(&report, "П-02", None).is_some());
    assert!(line(&report, "П-05", None).is_some_and(|l| l.kind == op_model::PriceKind::Material));
}

#[test]
fn estimate_and_prices_files_round_trip() {
    use crate::protocol::TableFormat;
    let mut s = state();
    finished(&mut s);
    s.handle(ToEngine::DefaultPrices);
    let book = s.estimate_file(TableFormat::Xlsx).unwrap();
    let rows = op_export::xlsx::read(&book).unwrap();
    assert!(
        rows.iter()
            .any(|r| r.first().is_some_and(|c| c == "Итог сметы"))
    );
    let csv = String::from_utf8(s.estimate_file(TableFormat::Csv).unwrap()).unwrap();
    assert!(csv.starts_with('\u{feff}') && csv.contains("Список закупок"));
    // Прайс уходит в CSV и приходит обратно в новый проект.
    let prices = s.prices_file(TableFormat::Csv).unwrap();
    let mut other = state();
    other.import_prices(&prices, "02.11.2026").unwrap();
    assert_eq!(other.document().prices.len(), s.document().prices.len());
    // Новая цена из XLSX — с историей.
    let mut changed = other.document().prices.clone();
    changed[0].price += 100.0;
    let book = op_export::xlsx::write(&[op_export::xlsx::Worksheet {
        name: "Прайс".into(),
        tables: vec![crate::estimate::prices_table(&changed)],
    }])
    .unwrap();
    other.import_prices(&book, "03.11.2026").unwrap();
    let first = &other.document().prices[0];
    assert_eq!(first.history.len(), 1);
    assert_eq!(first.history[0].date, "03.11.2026");
    assert!(other.import_prices(b"nothing here", "").is_err());
}

// ---------------------------------------------------------------- IFC

#[test]
fn ifc_export_has_walls_openings_spaces_and_furniture() {
    use crate::protocol::DrawingFormat;
    let mut s = state();
    let variant = replanned(&mut s);
    furniture_tool(&mut s, "sofa");
    click(&mut s, 2000.0, 4000.0);
    let survey = String::from_utf8(
        s.export_drawings(&request(None), DrawingFormat::Ifc)
            .unwrap(),
    )
    .unwrap();
    let count = |text: &str, entity: &str| text.matches(entity).count();
    // Обмер: 5 стен, дверь, окно, два помещения; мебель варианта сюда не входит.
    assert_eq!(count(&survey, "IFCWALL("), 5);
    assert_eq!(count(&survey, "IFCDOOR("), 1);
    assert_eq!(count(&survey, "IFCWINDOW("), 1);
    assert_eq!(count(&survey, "IFCSPACE("), 2);
    assert_eq!(count(&survey, "IFCFURNITURE("), 0);
    let replan = String::from_utf8(
        s.export_drawings(&request(Some(&variant)), DrawingFormat::Ifc)
            .unwrap(),
    )
    .unwrap();
    assert!(replan.contains("IFCFURNITURE("));
    assert!(
        replan.contains("\\X2\\"),
        "кириллица кодируется по ISO 10303-21"
    );
    let info = s.drawings(&request(Some(&variant))).preview();
    assert!(info.file_names.ifc.ends_with(" — модель.ifc"));
}

// ---------------------------------------------------------------- этажи

fn current_level(s: &EngineState) -> Option<String> {
    s.level.clone()
}

#[test]
fn levels_have_own_plans_and_add_up() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::DefaultPrices);
    let events = s.handle(ToEngine::AddLevel {
        name: "Мансарда".into(),
    });
    assert!(!has_error(&events));
    let level = current_level(&s).unwrap();
    assert!(
        events
            .events
            .iter()
            .any(|e| matches!(e, FromEngine::LevelChanged { level: Some(l) } if *l == level))
    );
    let doc = s.document();
    assert!((doc.levels[0].elevation - 3000.0).abs() < 1e-9);
    // На новом этаже план пуст; новая комната — на нём.
    assert!(s.projected.walls.is_empty() && s.derived.regions.is_empty());
    rectangle(&mut s);
    s.handle(ToEngine::FindRooms);
    let doc = s.document();
    assert_eq!(doc.walls.len(), 5 + 4);
    assert_eq!(
        doc.nodes
            .iter()
            .filter(|n| n.level.as_deref() == Some(level.as_str()))
            .count(),
        4
    );
    assert_eq!(s.projected.walls.len(), 4);
    // Сводка и смета — по всем этажам, помещения с названием этажа.
    let views = summaries(&s.snapshot());
    let survey = &views[0];
    assert_eq!(survey.rooms.len(), 3);
    assert!(
        survey
            .rooms
            .iter()
            .any(|r| r.name.starts_with("Мансарда · "))
    );
    assert!(survey.rooms.iter().any(|r| r.name.starts_with("Этаж 1 · ")));
    let report = estimate(&s.snapshot());
    assert!(
        report
            .rooms
            .iter()
            .any(|g| g.name.starts_with("Мансарда · "))
    );
    assert!(report.rooms.iter().any(|g| g.name.starts_with("Этаж 1 · ")));
    // В 3D нижний этаж — ниже текущего.
    enable_3d(&mut s);
    let scene = s.take_scene_3d().unwrap();
    let low = scene
        .opaque
        .vertices
        .iter()
        .map(|v| v.pos[2])
        .fold(f32::INFINITY, f32::min);
    assert!(low < -2000.0, "{low}");
    s.handle(ToEngine::Set3d {
        options: crate::protocol::Options3d::default(),
    });
    // Чертежи этажа и IFC со всеми этажами.
    let mut req = request(None);
    req.level = Some(level.clone());
    let set = s.drawings(&req);
    assert!(set.name.contains("Мансарда"), "{}", set.name);
    let ifc = String::from_utf8(
        s.export_drawings(&request(None), crate::protocol::DrawingFormat::Ifc)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ifc.matches("IFCBUILDINGSTOREY(").count(), 2);
    // Первый этаж — свой план.
    s.handle(ToEngine::SetLevel { level: None });
    assert_eq!(s.projected.walls.len(), 5);
    assert!(has_error(&s.handle(ToEngine::SetLevel {
        level: Some("нет".into())
    })));
}

#[test]
fn deleting_a_level_removes_its_elements_in_one_step() {
    let mut s = state();
    two_rooms(&mut s);
    s.handle(ToEngine::AddLevel {
        name: String::new(),
    });
    let level = current_level(&s).unwrap();
    assert_eq!(s.document().levels[0].name, "Этаж 2");
    rectangle(&mut s);
    furniture_tool(&mut s, "sofa");
    click(&mut s, 2000.0, 3000.0);
    let before = s.document().walls.len();
    s.handle(ToEngine::DeleteLevel { id: level });
    let doc = s.document();
    assert!(doc.levels.is_empty() && doc.furniture.is_empty());
    assert_eq!(doc.walls.len(), before - 4);
    assert!(doc.nodes.iter().all(|n| n.level.is_none()));
    assert_eq!(current_level(&s), None);
    s.handle(ToEngine::Undo);
    assert_eq!(s.document().walls.len(), before);
    assert_eq!(s.document().furniture.len(), 1);
}

// ---------------------------------------------------------------- климат

fn climate(events: &EngineEvents) -> crate::protocol::ClimateReport {
    events
        .events
        .iter()
        .rev()
        .find_map(|e| match e {
            FromEngine::Climate { report } => Some(report.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn floor_heating_ventilation_and_air_conditioning() {
    use op_model::DeviceKind as K;
    let mut s = state();
    two_rooms(&mut s);
    let room = left_room(&s);
    furniture_tool(&mut s, "bed-double");
    click(&mut s, 1750.0, 3000.0);
    let bed = s.document().furniture[0].clone();
    device_tool(&mut s, K::Thermostat);
    click(&mut s, 1100.0, 2000.0);
    device_tool(&mut s, K::Breather);
    click(&mut s, 1700.0, 1150.0);
    device_tool(&mut s, K::AcIndoor);
    click(&mut s, 3200.0, 1150.0);
    tool(&mut s, Tool::Select);
    let thermostat = s
        .document()
        .devices
        .iter()
        .find(|d| d.kind == K::Thermostat)
        .unwrap()
        .id
        .clone();
    let events = s.handle(ToEngine::Command {
        command: Command::SetHeating {
            heating: vec![op_model::HeatingZone {
                id: "z1".into(),
                room: room.clone(),
                density: 150.0,
                margin: 100.0,
                thermostat: Some(thermostat.clone()),
            }],
        },
    });
    let report = climate(&events);
    let zone = &report.zones[0];
    // 1380 × 3880 мм минус отступ по периметру 10,52 м × 0,1 м минус кровать.
    let expected = 5.3544 - 1.052 - bed.width * bed.depth / 1e6;
    assert!(
        (zone.heated - (expected * 100.0).round() / 100.0).abs() < 0.011,
        "{zone:?}"
    );
    assert!((zone.power - (zone.heated * 150.0).round()).abs() < 1e-9);
    // Бризер — 120 м³/ч при нужных 3 м³/ч на м²; в правой комнате — кондиционер.
    let left = report.rooms.iter().find(|r| r.id == room).unwrap();
    assert!((left.air_provided - 120.0).abs() < 1e-9 && left.air_required > 0.0);
    let right = report.rooms.iter().find(|r| r.id != room).unwrap();
    assert_eq!(right.ac_units, 1);
    assert!(right.cooling > 0.0);
    // Группы: тёплый пол и кондиционер — своими группами, мощность пола — от зоны.
    let report = electrical(&s.handle(ToEngine::AutoCircuits));
    let heating = report
        .circuits
        .iter()
        .find(|c| c.purpose.starts_with("Тёплый пол"))
        .unwrap();
    assert!((heating.installed - zone.power).abs() < 1e-9, "{heating:?}");
    assert!(
        report
            .circuits
            .iter()
            .any(|c| c.purpose.starts_with("Кондиционер"))
    );
    // Лист, смета.
    let set = s.drawings(&request(None));
    assert!(
        set.sheets
            .iter()
            .any(|x| x.title == "План тёплых полов и климата")
    );
    s.handle(ToEngine::DefaultPrices);
    let est = estimate(&s.snapshot());
    let mat = est.lines.iter().find(|l| l.code == "К-02").unwrap();
    assert!((mat.quantity - zone.heated).abs() < 0.011);
    // Удалённый терморегулятор отвязывается, удалённое помещение уносит зону.
    s.handle(ToEngine::Select {
        ids: vec![thermostat],
    });
    s.handle(ToEngine::DeleteSelection);
    assert!(s.document().heating[0].thermostat.is_none());
    s.handle(ToEngine::Select { ids: vec![room] });
    s.handle(ToEngine::DeleteSelection);
    assert!(s.document().heating.is_empty());
}

// ---------------------------------------------------------------- рендер

#[test]
fn render_queue_input_and_shots_in_the_project() {
    let mut s = state();
    assert!(s.render_input(None).is_err());
    two_rooms(&mut s);
    let input = s.render_input(None).unwrap();
    let png = crate::render::trace_png(&input, 32, 24, 2, |_| true).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    // Ракурс из проекта.
    enable_3d(&mut s);
    s.handle(ToEngine::SaveView3d {
        name: "Гостиная".into(),
    });
    let view = s.document().views[0].id.clone();
    assert!(s.render_input(Some(&view)).is_ok());
    assert!(s.render_input(Some("нет")).is_err());
    // Кадр сохраняется в проекте и удаляется вместе с файлом, одним шагом каждый.
    s.add_render(png.clone(), "Гостиная", 2, "02.11.2026")
        .unwrap();
    let shot = s.document().renders[0].clone();
    assert_eq!((shot.width, shot.height), (32, 24));
    assert_eq!(s.asset_bytes(&shot.asset).unwrap(), png);
    s.handle(ToEngine::DeleteRender {
        id: shot.id.clone(),
    });
    assert!(s.document().renders.is_empty() && s.asset_bytes(&shot.asset).is_none());
    s.handle(ToEngine::Undo);
    assert_eq!(s.document().renders.len(), 1);
    // Файл проекта уносит кадр с собой.
    let bytes = s.export_file("test").unwrap();
    let mut other = state();
    other.import_file(&bytes).unwrap();
    assert!(other.asset_bytes(&shot.asset).is_some());
    assert!(s.add_render(b"not png".to_vec(), "x", 1, "").is_err());
}
