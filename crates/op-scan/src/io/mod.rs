//! Чтение файлов скана: GLB и glTF, OBJ, PLY, ZIP с ними и параметрическая
//! комната RoomPlan (JSON).

mod glb;
mod obj;
mod ply;
mod roomplan;

use std::io::{Cursor, Read};

use glam::DVec3;

pub use glb::write_glb;
pub use roomplan::{RoomKind, RoomObject, RoomPlan};

use crate::{ScanError, ScanFormat};

/// Сетка: вершины и треугольники.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    /// Вершины в единицах файла.
    pub positions: Vec<DVec3>,
    /// Треугольники — индексы вершин.
    pub triangles: Vec<[u32; 3]>,
}

/// Облако точек.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cloud {
    /// Точки в единицах файла.
    pub points: Vec<DVec3>,
    /// Нормали, если есть в файле.
    pub normals: Option<Vec<DVec3>>,
}

/// Направление «вверх» в системе координат файла.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Up {
    /// Ось +Y (glTF, OBJ, RoomPlan).
    Y,
    /// Определить по данным (PLY).
    Detect,
}

/// Содержимое файла скана.
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    /// Формат основной геометрии.
    pub format: Option<ScanFormat>,
    /// Сетка.
    pub mesh: Mesh,
    /// Облако точек.
    pub cloud: Cloud,
    /// Параметрическая комната.
    pub room: Option<RoomPlan>,
    /// Направление «вверх».
    pub up: Option<Up>,
    /// Множитель в миллиметры (`None` — определить по размеру).
    pub to_mm: Option<f64>,
}

fn starts_with(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.len() >= prefix.len() && &bytes[..prefix.len()] == prefix
}

fn looks_like_obj(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    head.lines().any(|l| l.trim_start().starts_with("v "))
}

/// Читает файл скана по содержимому (расширение — подсказка для сообщений).
///
/// # Errors
///
/// Файл повреждён, формат не поддерживается или в нём нет геометрии.
pub fn load(bytes: &[u8]) -> Result<Loaded, ScanError> {
    let mut loaded = Loaded::default();
    add(&mut loaded, bytes, 0)?;
    if loaded.mesh.triangles.is_empty() && loaded.cloud.points.is_empty() && loaded.room.is_none() {
        return Err(ScanError::Empty);
    }
    Ok(loaded)
}

fn add(loaded: &mut Loaded, bytes: &[u8], depth: u32) -> Result<(), ScanError> {
    if starts_with(bytes, b"glTF") {
        let mesh = glb::read(bytes)?;
        set_geometry(loaded, ScanFormat::Glb, Up::Y, Some(1000.0));
        append_mesh(&mut loaded.mesh, mesh);
    } else if starts_with(bytes, b"ply") {
        let (mesh, cloud) = ply::read(bytes)?;
        set_geometry(loaded, ScanFormat::Ply, Up::Detect, None);
        append_mesh(&mut loaded.mesh, mesh);
        loaded.cloud = cloud;
    } else if starts_with(bytes, b"PK\x03\x04") {
        if depth > 0 {
            return Ok(());
        }
        read_zip(loaded, bytes)?;
    } else if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') && glb::is_gltf(bytes)
    {
        let mesh = glb::read_gltf(bytes)?;
        set_geometry(loaded, ScanFormat::Glb, Up::Y, Some(1000.0));
        append_mesh(&mut loaded.mesh, mesh);
    } else if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        loaded.room = Some(roomplan::read(bytes)?);
        if loaded.format.is_none() {
            loaded.format = Some(ScanFormat::RoomPlan);
        }
    } else if looks_like_obj(bytes) {
        let mesh = obj::read(bytes)?;
        set_geometry(loaded, ScanFormat::Obj, Up::Y, Some(1000.0));
        append_mesh(&mut loaded.mesh, mesh);
    } else {
        return Err(ScanError::Unsupported);
    }
    Ok(())
}

fn set_geometry(loaded: &mut Loaded, format: ScanFormat, up: Up, to_mm: Option<f64>) {
    if matches!(loaded.format, None | Some(ScanFormat::RoomPlan)) {
        loaded.format = Some(format);
        loaded.up = Some(up);
        loaded.to_mm = to_mm;
    }
}

fn append_mesh(target: &mut Mesh, mesh: Mesh) {
    let base = u32::try_from(target.positions.len()).unwrap_or(u32::MAX);
    target.positions.extend(mesh.positions);
    target.triangles.extend(
        mesh.triangles
            .into_iter()
            .map(|[a, b, c]| [a + base, b + base, c + base]),
    );
}

/// Наибольший размер распакованного файла внутри архива.
const MAX_ENTRY: u64 = 1 << 30;

fn read_zip(loaded: &mut Loaded, bytes: &[u8]) -> Result<(), ScanError> {
    let mut zip =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ScanError::Corrupt("ZIP"))?;
    // Геометрия: самый большой файл сетки или облака; JSON — данные комнаты.
    let mut geometry: Option<(u64, usize)> = None;
    let mut room = None;
    for i in 0..zip.len() {
        let Ok(file) = zip.by_index(i) else {
            continue;
        };
        let Ok(name) = file.name() else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        if name.starts_with("__macosx") || file.is_dir() {
            continue;
        }
        let size = file.size();
        if [".glb", ".obj", ".ply"].iter().any(|e| name.ends_with(e)) {
            if geometry.is_none_or(|(s, _)| size > s) {
                geometry = Some((size, i));
            }
        } else if name.ends_with(".json") && room.is_none() {
            room = Some(i);
        }
    }
    let mut read = |i: usize| -> Result<Vec<u8>, ScanError> {
        let file = zip.by_index(i).map_err(|_| ScanError::Corrupt("ZIP"))?;
        if file.size() > MAX_ENTRY {
            return Err(ScanError::TooLarge);
        }
        let mut data = Vec::new();
        file.take(MAX_ENTRY).read_to_end(&mut data)?;
        Ok(data)
    };
    if let Some((_, i)) = geometry {
        let data = read(i)?;
        add(loaded, &data, 1)?;
    }
    if let Some(i) = room {
        let data = read(i)?;
        // Посторонний JSON (метаданные экспорта) — не ошибка.
        if let Ok(parsed) = roomplan::read(&data) {
            loaded.room = Some(parsed);
            if loaded.format.is_none() {
                loaded.format = Some(ScanFormat::RoomPlan);
            }
        }
    }
    if geometry.is_none() && loaded.room.is_none() {
        return Err(ScanError::Unsupported);
    }
    Ok(())
}

pub(crate) fn f32_at(bytes: &[u8], at: usize, big_endian: bool) -> Option<f32> {
    let b: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(if big_endian {
        f32::from_be_bytes(b)
    } else {
        f32::from_le_bytes(b)
    })
}
