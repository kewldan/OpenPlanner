//! Wavefront OBJ: вершины `v` и грани `f` (многоугольники — веером).

use glam::DVec3;

use super::Mesh;
use crate::ScanError;

pub(super) fn read(bytes: &[u8]) -> Result<Mesh, ScanError> {
    let text = String::from_utf8_lossy(bytes);
    let mut mesh = Mesh::default();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let v: Vec<f64> = parts.take(3).filter_map(|p| p.parse().ok()).collect();
                if let [x, y, z] = v[..] {
                    mesh.positions.push(DVec3::new(x, y, z));
                } else {
                    return Err(ScanError::Corrupt("OBJ: вершина"));
                }
            }
            Some("f") => {
                let n = i64::try_from(mesh.positions.len()).map_err(|_| ScanError::TooLarge)?;
                let idx: Vec<u32> = parts
                    .filter_map(|p| p.split('/').next()?.parse::<i64>().ok())
                    .filter_map(|i| {
                        let i = if i < 0 { n + i } else { i - 1 };
                        u32::try_from(i).ok().filter(|&i| i64::from(i) < n)
                    })
                    .collect();
                for k in 1..idx.len().saturating_sub(1) {
                    mesh.triangles.push([idx[0], idx[k], idx[k + 1]]);
                }
            }
            _ => {}
        }
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_becomes_two_triangles() {
        let mesh = read(b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1/1 2/2 3/3 4/4\n").unwrap();
        assert_eq!(mesh.triangles, vec![[0, 1, 2], [0, 2, 3]]);
    }
}
