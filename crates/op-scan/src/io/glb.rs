//! glTF 2.0 Binary: сетки сцены с учётом трансформаций узлов.

use glam::{DAffine3, DMat4, DQuat, DVec3};
use serde_json::Value;

use super::{Mesh, f32_at};
use crate::ScanError;

const JSON: u32 = 0x4E4F_534A;
const BIN: u32 = 0x004E_4942;

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn index(v: &Value) -> Option<usize> {
    v.as_u64().and_then(|n| usize::try_from(n).ok())
}

fn numbers(v: &Value) -> Option<Vec<f64>> {
    v.as_array()?.iter().map(Value::as_f64).collect()
}

struct Glb<'a> {
    json: Value,
    bin: &'a [u8],
}

impl Glb<'_> {
    /// Байты и шаг данных аксессора.
    fn view(&self, accessor: &Value) -> Result<(&[u8], usize, usize), ScanError> {
        let corrupt = || ScanError::Corrupt("GLB");
        let view = self.json["bufferViews"]
            .get(index(&accessor["bufferView"]).ok_or(corrupt())?)
            .ok_or(corrupt())?;
        if index(&view["buffer"]) != Some(0) {
            return Err(ScanError::Corrupt("GLB: внешние буферы не поддерживаются"));
        }
        let start =
            index(&view["byteOffset"]).unwrap_or(0) + index(&accessor["byteOffset"]).unwrap_or(0);
        let length = index(&view["byteLength"]).ok_or(corrupt())?;
        let stride = index(&view["byteStride"]).unwrap_or(0);
        let count = index(&accessor["count"]).ok_or(corrupt())?;
        let end = (index(&view["byteOffset"]).unwrap_or(0) + length).min(self.bin.len());
        Ok((self.bin.get(start..end).ok_or(corrupt())?, stride, count))
    }

    fn positions(&self, accessor: &Value) -> Result<Vec<DVec3>, ScanError> {
        if accessor["componentType"].as_u64() != Some(5126) || accessor["type"] != "VEC3" {
            return Err(ScanError::Corrupt("GLB: координаты вершин не float3"));
        }
        let (data, stride, count) = self.view(accessor)?;
        let stride = if stride == 0 { 12 } else { stride };
        (0..count)
            .map(|i| {
                let at = i * stride;
                match (
                    f32_at(data, at, false),
                    f32_at(data, at + 4, false),
                    f32_at(data, at + 8, false),
                ) {
                    (Some(x), Some(y), Some(z)) => {
                        Ok(DVec3::new(f64::from(x), f64::from(y), f64::from(z)))
                    }
                    _ => Err(ScanError::Corrupt("GLB: аксессор выходит за буфер")),
                }
            })
            .collect()
    }

    fn indices(&self, accessor: &Value) -> Result<Vec<u32>, ScanError> {
        let (data, stride, count) = self.view(accessor)?;
        let size = match accessor["componentType"].as_u64() {
            Some(5121) => 1,
            Some(5123) => 2,
            Some(5125) => 4,
            _ => return Err(ScanError::Corrupt("GLB: тип индексов")),
        };
        let stride = if stride == 0 { size } else { stride };
        (0..count)
            .map(|i| {
                let at = i * stride;
                let b = data
                    .get(at..at + size)
                    .ok_or(ScanError::Corrupt("GLB: индексы"))?;
                Ok(match size {
                    1 => u32::from(b[0]),
                    2 => u32::from(u16::from_le_bytes([b[0], b[1]])),
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                })
            })
            .collect()
    }

    fn mesh(&self, index_: usize, transform: &DAffine3, out: &mut Mesh) -> Result<(), ScanError> {
        let Some(mesh) = self.json["meshes"].get(index_) else {
            return Ok(());
        };
        for primitive in mesh["primitives"].as_array().into_iter().flatten() {
            if primitive["extensions"]
                .get("KHR_draco_mesh_compression")
                .is_some()
            {
                return Err(ScanError::Corrupt("GLB: сжатие Draco не поддерживается"));
            }
            if index(&primitive["mode"]).unwrap_or(4) != 4 {
                continue;
            }
            let Some(pos) = index(&primitive["attributes"]["POSITION"]) else {
                continue;
            };
            let positions = self.positions(&self.json["accessors"][pos])?;
            let base = u32::try_from(out.positions.len()).map_err(|_| ScanError::TooLarge)?;
            let n = u32::try_from(positions.len()).map_err(|_| ScanError::TooLarge)?;
            out.positions
                .extend(positions.into_iter().map(|p| transform.transform_point3(p)));
            let indices = match index(&primitive["indices"]) {
                Some(i) => self.indices(&self.json["accessors"][i])?,
                None => (0..n).collect(),
            };
            for t in indices.as_chunks::<3>().0 {
                if t.iter().all(|&i| i < n) {
                    out.triangles.push([base + t[0], base + t[1], base + t[2]]);
                }
            }
        }
        Ok(())
    }

    fn node(
        &self,
        i: usize,
        parent: &DAffine3,
        out: &mut Mesh,
        depth: u32,
    ) -> Result<(), ScanError> {
        let Some(node) = self.json["nodes"].get(i) else {
            return Ok(());
        };
        if depth > 64 {
            return Err(ScanError::Corrupt("GLB: слишком глубокая иерархия"));
        }
        let local = if let Some(m) = numbers(&node["matrix"]).filter(|m| m.len() == 16) {
            DAffine3::from_mat4(DMat4::from_cols_slice(&m))
        } else {
            let t = numbers(&node["translation"])
                .filter(|v| v.len() == 3)
                .map_or(DVec3::ZERO, |v| DVec3::new(v[0], v[1], v[2]));
            let r = numbers(&node["rotation"])
                .filter(|v| v.len() == 4)
                .map_or(DQuat::IDENTITY, |v| {
                    DQuat::from_xyzw(v[0], v[1], v[2], v[3]).normalize()
                });
            let s = numbers(&node["scale"])
                .filter(|v| v.len() == 3)
                .map_or(DVec3::ONE, |v| DVec3::new(v[0], v[1], v[2]));
            DAffine3::from_scale_rotation_translation(s, r, t)
        };
        let transform = *parent * local;
        if let Some(m) = index(&node["mesh"]) {
            self.mesh(m, &transform, out)?;
        }
        for child in node["children"].as_array().into_iter().flatten() {
            if let Some(c) = index(child) {
                self.node(c, &transform, out, depth + 1)?;
            }
        }
        Ok(())
    }
}

/// Читает все треугольные сетки GLB в координатах сцены.
pub(super) fn read(bytes: &[u8]) -> Result<Mesh, ScanError> {
    let corrupt = || ScanError::Corrupt("GLB");
    if u32_at(bytes, 4) != Some(2) {
        return Err(ScanError::Corrupt("GLB: поддерживается только glTF 2.0"));
    }
    let mut at = 12;
    let mut json = None;
    let mut bin: &[u8] = &[];
    while at + 8 <= bytes.len() {
        let len = usize::try_from(u32_at(bytes, at).ok_or(corrupt())?).map_err(|_| corrupt())?;
        let kind = u32_at(bytes, at + 4).ok_or(corrupt())?;
        let data = bytes.get(at + 8..at + 8 + len).ok_or(corrupt())?;
        match kind {
            JSON => json = Some(serde_json::from_slice::<Value>(data).map_err(|_| corrupt())?),
            BIN => bin = data,
            _ => {}
        }
        at += 8 + len;
    }
    let glb = Glb {
        json: json.ok_or(corrupt())?,
        bin,
    };
    let mut mesh = Mesh::default();
    let scene = index(&glb.json["scene"]).unwrap_or(0);
    let roots: Vec<usize> = glb.json["scenes"]
        .get(scene)
        .and_then(|s| s["nodes"].as_array())
        .map(|nodes| nodes.iter().filter_map(index).collect())
        .unwrap_or_default();
    if roots.is_empty() {
        let count = glb.json["meshes"].as_array().map_or(0, Vec::len);
        for m in 0..count {
            glb.mesh(m, &DAffine3::IDENTITY, &mut mesh)?;
        }
    } else {
        for root in roots {
            glb.node(root, &DAffine3::IDENTITY, &mut mesh, 0)?;
        }
    }
    Ok(mesh)
}

/// Записывает сетку в GLB (одна сетка, один узел) — для тестовых сканов.
#[must_use]
pub fn write_glb(mesh: &Mesh) -> Vec<u8> {
    let mut bin = Vec::with_capacity(mesh.positions.len() * 12 + mesh.triangles.len() * 12);
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &mesh.positions {
        #[allow(clippy::cast_possible_truncation)] // GLB хранит float32
        let v = [p.x as f32, p.y as f32, p.z as f32];
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
            bin.extend_from_slice(&v[k].to_le_bytes());
        }
    }
    let positions_len = bin.len();
    for t in &mesh.triangles {
        for i in t {
            bin.extend_from_slice(&i.to_le_bytes());
        }
    }
    let json = serde_json::json!({
        "asset": { "version": "2.0", "generator": "OpenPlanner test scan" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 1 }] }],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": positions_len },
            { "buffer": 0, "byteOffset": positions_len, "byteLength": bin.len() - positions_len }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": mesh.positions.len(), "type": "VEC3", "min": lo, "max": hi },
            { "bufferView": 1, "componentType": 5125, "count": mesh.triangles.len() * 3, "type": "SCALAR" }
        ]
    });
    let mut json = serde_json::to_vec(&json).unwrap_or_default();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    let u = |n: usize| u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&u(total));
    out.extend_from_slice(&u(json.len()));
    out.extend_from_slice(&JSON.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&u(bin.len()));
    out.extend_from_slice(&BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mesh = Mesh {
            positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y],
            triangles: vec![[0, 1, 2]],
        };
        let back = read(&write_glb(&mesh)).unwrap();
        assert_eq!(back, mesh);
    }

    #[test]
    fn rejects_garbage() {
        assert!(read(b"glTF\x01\0\0\0").is_err());
    }
}
