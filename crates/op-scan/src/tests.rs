//! Распознавание синтетических сканов.

use glam::DVec2;
use op_model::{Justification, OpeningKind};

use crate::{io::write_glb, synth, *};

fn analyze_scene(scene: &synth::Scene) -> ScanResult {
    let glb = write_glb(&synth::mesh(scene));
    let mut stages = Vec::new();
    let result = analyze(&glb, &mut |s| stages.push(s)).unwrap();
    assert_eq!(stages.len(), usize::try_from(Stage::COUNT).unwrap());
    result
}

fn wall_length(w: &ScanWall) -> f64 {
    w.a.vec().distance(w.b.vec())
}

/// Площадь области стен плана, в которой лежит точка (через op-geom).
fn room_areas(plan: &ScanPlan) -> Vec<f64> {
    let inputs: Vec<op_geom::walls::WallInput> = plan
        .walls
        .iter()
        .map(|w| {
            let (left, right) = match w.justification {
                Justification::Center => (w.thickness / 2.0, w.thickness / 2.0),
                Justification::Left => (w.thickness, 0.0),
                Justification::Right => (0.0, w.thickness),
            };
            op_geom::walls::WallInput {
                a: w.a,
                b: w.b,
                left,
                right,
            }
        })
        .collect();
    let outlines = op_geom::walls::wall_outlines(&plan.nodes, &inputs);
    let mut areas: Vec<f64> = op_geom::shapes::regions(&op_geom::shapes::union(&outlines))
        .iter()
        .map(|r| r.area / 1e6)
        .collect();
    areas.sort_by(f64::total_cmp);
    areas
}

#[test]
fn apartment_is_recognised() {
    let result = analyze_scene(&synth::apartment(0.0));
    assert_eq!(result.failure, None);
    assert!(result.ceiling_detected);
    assert!(
        (result.ceiling_height - 2700.0).abs() <= 20.0,
        "{}",
        result.ceiling_height
    );

    // Шкаф — «лишняя стена?», принятая по умолчанию.
    let extra: Vec<_> = result
        .issues
        .iter()
        .filter(|i| matches!(i.kind, IssueKind::ExtraWall { .. }))
        .collect();
    assert!(
        !extra.is_empty(),
        "шкаф должен вызвать вопрос: {:?}",
        result.walls
    );

    let plan = apply(&result, &[], &ScanParams::default());
    let paired: Vec<_> = result
        .walls
        .iter()
        .filter(|w| w.source == WallSource::Paired)
        .collect();
    assert_eq!(
        paired.len(),
        1,
        "перегородка с двух сторон: {:?}",
        result.walls
    );
    assert!(
        (paired[0].thickness - 120.0).abs() <= 20.0,
        "{}",
        paired[0].thickness
    );
    assert!((wall_length(paired[0]) - 4000.0).abs() <= 150.0);

    let areas = room_areas(&plan);
    assert_eq!(
        areas.len(),
        2,
        "две комнаты: {areas:?}; стены {:?}",
        plan.walls
    );
    assert!((areas[0] - 13.52).abs() < 0.25, "{areas:?}");
    assert!((areas[1] - 14.0).abs() < 0.25, "{areas:?}");

    let count = |k| plan.openings.iter().filter(|o| o.kind == k).count();
    assert_eq!(count(OpeningKind::Window), 2, "{:?}", plan.openings);
    assert_eq!(count(OpeningKind::Door), 2, "{:?}", plan.openings);
    for o in &plan.openings {
        let expected = match o.kind {
            OpeningKind::Window => 1200.0,
            _ => 850.0,
        };
        assert!((o.width - expected).abs() <= 120.0, "{o:?}");
    }
    let window = plan
        .openings
        .iter()
        .find(|o| o.kind == OpeningKind::Window)
        .unwrap();
    assert!((window.sill - 850.0).abs() <= 120.0, "{window:?}");
}

#[test]
fn rotated_scan_is_aligned() {
    let result = analyze_scene(&synth::apartment(0.3));
    assert_eq!(result.failure, None);
    for w in &result.walls {
        let d = w.b.vec() - w.a.vec();
        let off_axis = d.x.abs().min(d.y.abs()) / d.length();
        assert!(off_axis < 0.02, "стена не по осям: {w:?}");
    }
    let plan = apply(&result, &[], &ScanParams::default());
    assert_eq!(room_areas(&plan).len(), 2);
}

#[test]
fn rejecting_extra_wall_keeps_it() {
    let result = analyze_scene(&synth::apartment(0.0));
    let issue = result
        .issues
        .iter()
        .find(|i| matches!(i.kind, IssueKind::ExtraWall { .. }))
        .unwrap();
    let default = apply(&result, &[], &ScanParams::default());
    let kept = apply(
        &result,
        &[ScanAnswer {
            issue: issue.id,
            accept: false,
        }],
        &ScanParams::default(),
    );
    assert_eq!(kept.walls.len(), default.walls.len() + 1);
}

#[test]
fn single_wall_thickness_is_a_parameter() {
    let result = analyze_scene(&synth::apartment(0.0));
    let params = ScanParams {
        single_thickness: 380.0,
        ..ScanParams::default()
    };
    let plan = apply(&result, &[], &params);
    assert!(
        plan.walls
            .iter()
            .any(|w| (w.thickness - 380.0).abs() < 1e-9)
    );
    // Толщина наружу не меняет площадь комнат.
    let areas = room_areas(&plan);
    assert!((areas[1] - 14.0).abs() < 0.25, "{areas:?}");
}

#[test]
fn missing_wall_is_suggested() {
    // Одна комната без правой стены: свободные концы верхней и нижней стен.
    let mut scene = synth::apartment(0.0);
    scene.rooms.truncate(1);
    scene.rooms[0].holes.clear();
    scene.rooms[0].furniture.clear();
    let mut mesh = synth::mesh(&scene);
    // Убираем треугольники правой стены (x = 3500 мм → координата Z файла = 3,5 м).
    let keep: Vec<[u32; 3]> = mesh
        .triangles
        .iter()
        .copied()
        .filter(|t| {
            !t.iter()
                .all(|&i| (mesh.positions[i as usize].z - 3.5).abs() < 0.02)
        })
        .collect();
    mesh.triangles = keep;
    let result = analyze(&write_glb(&mesh), &mut |_| {}).unwrap();
    let missing = result
        .issues
        .iter()
        .find(|i| matches!(i.kind, IssueKind::MissingWall { .. }))
        .unwrap_or_else(|| panic!("нет вопроса: {:?}", result.issues));
    let open = apply(
        &result,
        &[ScanAnswer {
            issue: missing.id,
            accept: false,
        }],
        &ScanParams::default(),
    );
    let closed = apply(
        &result,
        &[ScanAnswer {
            issue: missing.id,
            accept: true,
        }],
        &ScanParams::default(),
    );
    assert!(room_areas(&open).is_empty());
    assert_eq!(room_areas(&closed).len(), 1);
}

#[test]
fn ply_point_cloud_with_detected_up_axis() {
    // Облако из вершин сетки в Z-up (как у многих экспортов PLY), без нормалей.
    let mesh = synth::mesh(&synth::apartment(0.0));
    let samples = {
        let loaded = io::Loaded {
            format: Some(ScanFormat::Glb),
            mesh: mesh.clone(),
            up: Some(io::Up::Y),
            to_mm: Some(1000.0),
            ..Default::default()
        };
        samples::build(&loaded)
    };
    let mut ply = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nend_header\n",
        samples.len()
    );
    for s in &samples {
        // План → Z-up в метрах: x, −y, высота.
        ply += &format!(
            "{} {} {}\n",
            s.p.x / 1000.0,
            -s.p.y / 1000.0,
            s.p.z / 1000.0
        );
    }
    let result = analyze(ply.as_bytes(), &mut |_| {}).unwrap();
    assert_eq!(result.source.format, ScanFormat::Ply);
    assert!(
        (result.ceiling_height - 2700.0).abs() <= 30.0,
        "{}",
        result.ceiling_height
    );
    let plan = apply(&result, &[], &ScanParams::default());
    let areas = room_areas(&plan);
    assert_eq!(areas.len(), 2, "{areas:?}");
}

#[test]
fn zip_with_glb_and_room_data() {
    let glb = write_glb(&synth::mesh(&synth::apartment(0.0)));
    // Параметрическая комната: четыре стены 4 × 3 м и дверь.
    let wall = |cx: f64, cz: f64, ax: f64, az: f64, w: f64| {
        serde_json::json!({
            "dimensions": [w, 2.7, 0.12], "confidence": {"high": {}}, "category": {"wall": {}},
            "transform": [ax, 0, az, 0,  0, 1, 0, 0,  -az, 0, ax, 0,  cx, 1.35, cz, 1]
        })
    };
    let json = serde_json::json!({
        "walls": [wall(2.0, 0.0, 1.0, 0.0, 4.0), wall(2.0, 3.0, 1.0, 0.0, 4.0), wall(0.0, 1.5, 0.0, 1.0, 3.0), wall(4.0, 1.5, 0.0, 1.0, 3.0)],
        "doors": [{"dimensions": [0.8, 2.0, 0.1], "confidence": "high", "category": {"door": {"isOpen": false}},
                   "transform": [1, 0, 0, 0,  0, 1, 0, 0,  0, 0, 1, 0,  2.0, 1.0, 3.0, 1]}]
    });
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("scan/mesh.glb", options).unwrap();
    std::io::Write::write_all(&mut zip, &glb).unwrap();
    zip.start_file("scan/room.json", options).unwrap();
    std::io::Write::write_all(&mut zip, json.to_string().as_bytes()).unwrap();
    let bytes = zip.finish().unwrap().into_inner();

    let result = analyze(&bytes, &mut |_| {}).unwrap();
    assert!(result.source.room_data);
    assert_eq!(result.walls.len(), 4);
    assert!(
        result
            .walls
            .iter()
            .all(|w| w.source == WallSource::RoomPlan)
    );
    assert_eq!(result.openings.len(), 1);
    assert!(result.raster.is_some(), "вид сверху из сетки");
    let plan = apply(&result, &[], &ScanParams::default());
    let areas = room_areas(&plan);
    assert_eq!(areas.len(), 1);
    assert!((areas[0] - 3.88 * 2.88).abs() < 0.05, "{areas:?}");
    let door = &plan.openings[0];
    assert_eq!(door.kind, OpeningKind::Door);
    assert!(
        (door.offset - 2000.0).abs() < 60.0 || (door.offset - 2000.0).abs() < 60.0,
        "{door:?}"
    );
}

#[test]
fn empty_room_fails_gracefully() {
    // Только пол: распознать стены нельзя, но вид сверху есть.
    let mut scene = synth::apartment(0.0);
    scene.rooms.truncate(1);
    let mut mesh = synth::mesh(&scene);
    let keep: Vec<[u32; 3]> = mesh
        .triangles
        .iter()
        .copied()
        .filter(|t| t.iter().all(|&i| mesh.positions[i as usize].y.abs() < 0.01))
        .collect();
    mesh.triangles = keep;
    let result = analyze(&write_glb(&mesh), &mut |_| {}).unwrap();
    assert_eq!(result.failure, Some(ScanFailure::TooFewWalls));
    assert!(result.raster.is_some());
}

#[test]
fn garbage_is_an_error() {
    assert!(matches!(
        analyze(b"hello", &mut |_| {}),
        Err(ScanError::Unsupported)
    ));
    assert!(analyze(b"glTF\x02\0\0\0", &mut |_| {}).is_err());
}

#[test]
fn raster_marks_walls_dark() {
    let result = analyze_scene(&synth::apartment(0.0));
    let raster = result.raster.unwrap();
    assert!(raster.png.starts_with(b"\x89PNG"));
    assert!(raster.width_px > 100 && raster.height_px > 100);
    let _ = DVec2::ZERO;
}
