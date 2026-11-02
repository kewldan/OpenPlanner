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
        .collect();
    assert_eq!(elements.len(), 7 + 4 + 4 + 1 + 1 + 1 + 1 + 1);
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
