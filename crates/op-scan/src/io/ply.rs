//! PLY: ASCII и двоичный, вершины (с нормалями, если есть) и грани.

use glam::DVec3;

use super::{Cloud, Mesh};
use crate::ScanError;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Kind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "char" | "int8" => Self::I8,
            "uchar" | "uint8" => Self::U8,
            "short" | "int16" => Self::I16,
            "ushort" | "uint16" => Self::U16,
            "int" | "int32" => Self::I32,
            "uint" | "uint32" => Self::U32,
            "float" | "float32" => Self::F32,
            "double" | "float64" => Self::F64,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

#[derive(Debug, Clone)]
enum Property {
    Scalar(String, Kind),
    List(String, Kind, Kind),
}

#[derive(Debug, Clone)]
struct Element {
    name: String,
    count: usize,
    properties: Vec<Property>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Encoding {
    Ascii,
    Little,
    Big,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    encoding: Encoding,
    tokens: std::str::SplitAsciiWhitespace<'a>,
}

impl Reader<'_> {
    fn value(&mut self, kind: Kind) -> Result<f64, ScanError> {
        let corrupt = || ScanError::Corrupt("PLY: данные короче заголовка");
        if self.encoding == Encoding::Ascii {
            return self
                .tokens
                .next()
                .and_then(|t| t.parse().ok())
                .ok_or(corrupt());
        }
        let size = kind.size();
        let b = self.data.get(self.at..self.at + size).ok_or(corrupt())?;
        self.at += size;
        let mut buf = [0u8; 8];
        buf[..size].copy_from_slice(b);
        if self.encoding == Encoding::Big {
            buf[..size].reverse();
        }
        Ok(match kind {
            Kind::I8 => f64::from(i8::from_le_bytes([buf[0]])),
            Kind::U8 => f64::from(buf[0]),
            Kind::I16 => f64::from(i16::from_le_bytes([buf[0], buf[1]])),
            Kind::U16 => f64::from(u16::from_le_bytes([buf[0], buf[1]])),
            Kind::I32 => f64::from(i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
            Kind::U32 => f64::from(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
            Kind::F32 => f64::from(f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
            Kind::F64 => f64::from_le_bytes(buf),
        })
    }
}

pub(super) fn read(bytes: &[u8]) -> Result<(Mesh, Cloud), ScanError> {
    let corrupt = |what| ScanError::Corrupt(what);
    let end = bytes
        .windows(10)
        .position(|w| w == b"end_header")
        .ok_or(corrupt("PLY: нет конца заголовка"))?;
    let header = std::str::from_utf8(&bytes[..end]).map_err(|_| corrupt("PLY: заголовок"))?;
    let mut body = end + 10;
    while body < bytes.len() && (bytes[body] == b'\r' || bytes[body] == b'\n') {
        body += 1;
        if bytes[body - 1] == b'\n' {
            break;
        }
    }
    let mut encoding = Encoding::Ascii;
    let mut elements: Vec<Element> = Vec::new();
    for line in header.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["format", "ascii", ..] => encoding = Encoding::Ascii,
            ["format", "binary_little_endian", ..] => encoding = Encoding::Little,
            ["format", "binary_big_endian", ..] => encoding = Encoding::Big,
            ["element", name, count] => elements.push(Element {
                name: (*name).to_owned(),
                count: count.parse().map_err(|_| corrupt("PLY: число элементов"))?,
                properties: Vec::new(),
            }),
            ["property", "list", count, item, name] => {
                let (Some(c), Some(i)) = (Kind::parse(count), Kind::parse(item)) else {
                    return Err(corrupt("PLY: тип свойства"));
                };
                elements
                    .last_mut()
                    .ok_or(corrupt("PLY: свойство вне элемента"))?
                    .properties
                    .push(Property::List((*name).to_owned(), c, i));
            }
            ["property", kind, name] => {
                let kind = Kind::parse(kind).ok_or(corrupt("PLY: тип свойства"))?;
                elements
                    .last_mut()
                    .ok_or(corrupt("PLY: свойство вне элемента"))?
                    .properties
                    .push(Property::Scalar((*name).to_owned(), kind));
            }
            _ => {}
        }
    }
    let text = if encoding == Encoding::Ascii {
        std::str::from_utf8(&bytes[body..]).map_err(|_| corrupt("PLY: текст"))?
    } else {
        ""
    };
    let mut reader = Reader {
        data: bytes,
        at: body,
        encoding,
        tokens: text.split_ascii_whitespace(),
    };
    let mut points = Vec::new();
    let mut normals = Vec::new();
    let mut triangles = Vec::new();
    let mut has_normals = false;
    for element in &elements {
        let is_vertex = element.name == "vertex";
        let is_face = element.name == "face";
        if is_vertex {
            has_normals = element
                .properties
                .iter()
                .any(|p| matches!(p, Property::Scalar(n, _) if n == "nx"));
        }
        for _ in 0..element.count {
            let mut v = [0.0f64; 6];
            for property in &element.properties {
                match property {
                    Property::Scalar(name, kind) => {
                        let value = reader.value(*kind)?;
                        if is_vertex {
                            let slot = match name.as_str() {
                                "x" => Some(0),
                                "y" => Some(1),
                                "z" => Some(2),
                                "nx" => Some(3),
                                "ny" => Some(4),
                                "nz" => Some(5),
                                _ => None,
                            };
                            if let Some(i) = slot {
                                v[i] = value;
                            }
                        }
                    }
                    Property::List(name, count, item) => {
                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        // размер списка — малое целое
                        let n = reader.value(*count)?.max(0.0) as usize;
                        let mut idx = Vec::with_capacity(n);
                        for _ in 0..n {
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            // индекс вершины
                            idx.push(reader.value(*item)?.max(0.0) as u32);
                        }
                        if is_face && (name == "vertex_indices" || name == "vertex_index") {
                            for k in 1..idx.len().saturating_sub(1) {
                                triangles.push([idx[0], idx[k], idx[k + 1]]);
                            }
                        }
                    }
                }
            }
            if is_vertex {
                points.push(DVec3::new(v[0], v[1], v[2]));
                normals.push(DVec3::new(v[3], v[4], v[5]));
            }
        }
    }
    let n = u32::try_from(points.len()).unwrap_or(u32::MAX);
    triangles.retain(|t: &[u32; 3]| t.iter().all(|&i| i < n));
    if triangles.is_empty() {
        Ok((
            Mesh::default(),
            Cloud {
                points,
                normals: has_normals.then_some(normals),
            },
        ))
    } else {
        Ok((
            Mesh {
                positions: points,
                triangles,
            },
            Cloud::default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_cloud_with_normals() {
        let ply = b"ply\nformat ascii 1.0\nelement vertex 2\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty float ny\nproperty float nz\nend_header\n1 2 3 0 0 1\n4 5 6 0 1 0\n";
        let (mesh, cloud) = read(ply).unwrap();
        assert!(mesh.triangles.is_empty());
        assert_eq!(cloud.points[1], DVec3::new(4.0, 5.0, 6.0));
        assert_eq!(cloud.normals.unwrap()[1], DVec3::Y);
    }

    #[test]
    fn binary_mesh() {
        let mut ply = b"ply\nformat binary_little_endian 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n".to_vec();
        for v in [[0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                ply.extend_from_slice(&c.to_le_bytes());
            }
        }
        ply.push(3);
        for i in [0i32, 1, 2] {
            ply.extend_from_slice(&i.to_le_bytes());
        }
        let (mesh, _) = read(&ply).unwrap();
        assert_eq!(mesh.triangles, vec![[0, 1, 2]]);
    }
}
