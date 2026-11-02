//! Мебель: каталог, условные обозначения на плане, детали для 3D, свои модели
//! GLB и привязка к стенам.
//!
//! Обозначения и формы нарисованы с нуля по размерам предмета (clean room):
//! символ строится в локальных осях — ширина вдоль X, глубина вдоль Y, спинка
//! на −Y, лицевая сторона на +Y, центр в нуле.

use glam::{DVec2, DVec3};
use op_geom::left_normal;
use op_model::{Document, Furniture};

use crate::{derived::Derived, protocol::CatalogEntry};

/// Раздел каталога.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// Спальня.
    Bedroom,
    /// Гостиная.
    Living,
    /// Кухня и столовая.
    Kitchen,
    /// Санузел.
    Bathroom,
    /// Хранение и прихожая.
    Storage,
    /// Рабочее место.
    Work,
}

impl Category {
    /// Название раздела.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bedroom => "Спальня",
            Self::Living => "Гостиная",
            Self::Kitchen => "Кухня и столовая",
            Self::Bathroom => "Санузел",
            Self::Storage => "Хранение и прихожая",
            Self::Work => "Рабочее место",
        }
    }
}

/// Позиция каталога.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogItem {
    /// Идентификатор.
    pub id: &'static str,
    /// Название.
    pub name: &'static str,
    /// Раздел.
    pub category: Category,
    /// Ширина, глубина, высота, мм.
    pub size: [f64; 3],
    /// Низ над полом, мм.
    pub elevation: f64,
    /// Слова для поиска.
    pub keywords: &'static str,
}

const fn item(
    id: &'static str,
    name: &'static str,
    category: Category,
    size: [f64; 3],
    elevation: f64,
    keywords: &'static str,
) -> CatalogItem {
    CatalogItem {
        id,
        name,
        category,
        size,
        elevation,
        keywords,
    }
}

/// Позиция для своей модели GLB.
pub const CUSTOM: &str = "custom";

/// Встроенный каталог.
pub const CATALOG: &[CatalogItem] = &[
    item(
        "bed-double",
        "Кровать двуспальная",
        Category::Bedroom,
        [1600.0, 2000.0, 450.0],
        0.0,
        "спальное место матрас",
    ),
    item(
        "bed-single",
        "Кровать односпальная",
        Category::Bedroom,
        [900.0, 2000.0, 450.0],
        0.0,
        "спальное место детская",
    ),
    item(
        "nightstand",
        "Тумба прикроватная",
        Category::Bedroom,
        [450.0, 400.0, 500.0],
        0.0,
        "тумбочка",
    ),
    item(
        "wardrobe",
        "Шкаф платяной",
        Category::Bedroom,
        [1800.0, 600.0, 2200.0],
        0.0,
        "гардероб купе одежда",
    ),
    item(
        "dresser",
        "Комод",
        Category::Bedroom,
        [1000.0, 450.0, 850.0],
        0.0,
        "ящики",
    ),
    item(
        "sofa",
        "Диван",
        Category::Living,
        [2200.0, 950.0, 850.0],
        0.0,
        "трёхместный",
    ),
    item(
        "sofa-corner",
        "Диван угловой",
        Category::Living,
        [2600.0, 1700.0, 850.0],
        0.0,
        "г-образный",
    ),
    item(
        "armchair",
        "Кресло",
        Category::Living,
        [850.0, 850.0, 850.0],
        0.0,
        "",
    ),
    item(
        "coffee-table",
        "Журнальный столик",
        Category::Living,
        [1100.0, 600.0, 450.0],
        0.0,
        "стол",
    ),
    item(
        "tv-stand",
        "Тумба под ТВ",
        Category::Living,
        [1600.0, 450.0, 500.0],
        0.0,
        "телевизор",
    ),
    item(
        "bookcase",
        "Стеллаж",
        Category::Living,
        [900.0, 350.0, 2000.0],
        0.0,
        "книги полки",
    ),
    item(
        "kitchen-base",
        "Кухонный шкаф нижний",
        Category::Kitchen,
        [600.0, 600.0, 900.0],
        0.0,
        "тумба столешница",
    ),
    item(
        "kitchen-wall",
        "Кухонный шкаф навесной",
        Category::Kitchen,
        [600.0, 350.0, 700.0],
        1450.0,
        "верхний",
    ),
    item(
        "kitchen-sink",
        "Мойка с тумбой",
        Category::Kitchen,
        [800.0, 600.0, 900.0],
        0.0,
        "раковина",
    ),
    item(
        "stove",
        "Плита",
        Category::Kitchen,
        [600.0, 600.0, 900.0],
        0.0,
        "варочная духовка",
    ),
    item(
        "fridge",
        "Холодильник",
        Category::Kitchen,
        [600.0, 650.0, 1850.0],
        0.0,
        "",
    ),
    item(
        "dining-table",
        "Обеденный стол",
        Category::Kitchen,
        [1400.0, 800.0, 750.0],
        0.0,
        "стол",
    ),
    item(
        "round-table",
        "Стол круглый",
        Category::Kitchen,
        [1000.0, 1000.0, 750.0],
        0.0,
        "обеденный",
    ),
    item(
        "chair",
        "Стул",
        Category::Kitchen,
        [450.0, 500.0, 900.0],
        0.0,
        "",
    ),
    item(
        "bathtub",
        "Ванна",
        Category::Bathroom,
        [1700.0, 750.0, 580.0],
        0.0,
        "",
    ),
    item(
        "shower",
        "Душевая кабина",
        Category::Bathroom,
        [900.0, 900.0, 2000.0],
        0.0,
        "поддон",
    ),
    item(
        "toilet",
        "Унитаз",
        Category::Bathroom,
        [380.0, 650.0, 800.0],
        0.0,
        "инсталляция",
    ),
    item(
        "washbasin",
        "Раковина",
        Category::Bathroom,
        [600.0, 450.0, 850.0],
        0.0,
        "умывальник",
    ),
    item(
        "washer",
        "Стиральная машина",
        Category::Bathroom,
        [600.0, 550.0, 850.0],
        0.0,
        "стиралка",
    ),
    item(
        "hallway-wardrobe",
        "Шкаф в прихожую",
        Category::Storage,
        [1200.0, 450.0, 2200.0],
        0.0,
        "гардероб",
    ),
    item(
        "shoe-cabinet",
        "Обувница",
        Category::Storage,
        [800.0, 300.0, 1000.0],
        0.0,
        "обувь",
    ),
    item(
        "shelf",
        "Полка навесная",
        Category::Storage,
        [800.0, 250.0, 300.0],
        1500.0,
        "",
    ),
    item(
        "desk",
        "Письменный стол",
        Category::Work,
        [1200.0, 600.0, 750.0],
        0.0,
        "компьютерный",
    ),
    item(
        "office-chair",
        "Кресло рабочее",
        Category::Work,
        [600.0, 600.0, 1000.0],
        0.0,
        "стул",
    ),
];

/// Позиция каталога по идентификатору.
#[must_use]
pub fn catalog_item(id: &str) -> Option<&'static CatalogItem> {
    CATALOG.iter().find(|i| i.id == id)
}

/// Линия обозначения в локальных осях предмета.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// Точки.
    pub points: Vec<DVec2>,
    /// Замкнутая.
    pub closed: bool,
    /// Штриховая (то, что выше уровня среза плана: навесные шкафы).
    pub dashed: bool,
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
    vec![
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ]
}

/// Прямоугольник со скруглёнными углами радиуса `r`.
fn rounded(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Vec<DVec2> {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut out = Vec::new();
    let corners = [
        (DVec2::new(x1 - r, y0 + r), -90.0),
        (DVec2::new(x1 - r, y1 - r), 0.0),
        (DVec2::new(x0 + r, y1 - r), 90.0),
        (DVec2::new(x0 + r, y0 + r), 180.0),
    ];
    for (c, start) in corners {
        for i in 0..=4 {
            let a = (start + f64::from(i) * 22.5_f64).to_radians();
            out.push(c + DVec2::new(a.cos(), a.sin()) * r);
        }
    }
    out
}

fn ellipse(c: DVec2, rx: f64, ry: f64) -> Vec<DVec2> {
    (0..32)
        .map(|i| {
            let a = f64::from(i) / 32.0 * std::f64::consts::TAU;
            c + DVec2::new(a.cos() * rx, a.sin() * ry)
        })
        .collect()
}

fn line(a: DVec2, b: DVec2) -> Mark {
    Mark {
        points: vec![a, b],
        closed: false,
        dashed: false,
    }
}

fn shape(points: Vec<DVec2>) -> Mark {
    Mark {
        points,
        closed: true,
        dashed: false,
    }
}

/// Условное обозначение предмета: первый контур — габарит.
#[must_use]
pub fn symbol(item: &str, w: f64, d: f64) -> Vec<Mark> {
    let (hx, hy) = (w / 2.0, d / 2.0);
    let (x0, y0, x1, y1) = (-hx, -hy, hx, hy);
    let p = DVec2::new;
    let mut out = vec![shape(rect(x0, y0, x1, y1))];
    match item {
        "bed-double" | "bed-single" => {
            let pillows = if item == "bed-double" { 2 } else { 1 };
            let pw = (w - 120.0 - 60.0 * f64::from(pillows - 1)) / f64::from(pillows);
            for i in 0..pillows {
                let x = x0 + 60.0 + f64::from(i) * (pw + 60.0);
                out.push(shape(rounded(
                    x,
                    y0 + 60.0,
                    x + pw,
                    y0 + 60.0 + d * 0.15,
                    60.0,
                )));
            }
            // Одеяло с отогнутым углом.
            let fold = y0 + d * 0.32;
            out.push(line(p(x0, fold), p(x1, fold)));
            out.push(line(p(x1 - w * 0.3, fold), p(x1, fold + w * 0.3)));
        }
        "nightstand" | "dresser" | "shoe-cabinet" => {
            out.push(shape(rect(x0 + 30.0, y0 + 30.0, x1 - 30.0, y1 - 30.0)));
            if item == "dresser" {
                out.push(line(p(0.0, y0 + 30.0), p(0.0, y1 - 30.0)));
            }
        }
        "wardrobe" | "hallway-wardrobe" => {
            out.push(line(p(x0, y1 - 30.0), p(x1, y1 - 30.0)));
            out.push(line(p(x0, y0), p(x1, y1 - 30.0)));
            out.push(line(p(x1, y0), p(x0, y1 - 30.0)));
        }
        "sofa" | "armchair" => {
            let arm = (w * 0.1).clamp(120.0, 220.0);
            let back = (d * 0.22).clamp(150.0, 250.0);
            out.push(shape(rect(x0, y0, x1, y0 + back)));
            out.push(shape(rect(x0, y0 + back, x0 + arm, y1)));
            out.push(shape(rect(x1 - arm, y0 + back, x1, y1)));
            let seats: u8 = if item == "sofa" { 3 } else { 1 };
            let sw = (w - 2.0 * arm) / f64::from(seats);
            for i in 1..seats {
                let x = x0 + arm + sw * f64::from(i);
                out.push(line(p(x, y0 + back), p(x, y1)));
            }
        }
        "sofa-corner" => {
            // Г-образный: длинная часть вдоль спинки, оттоманка справа.
            let seat = (d * 0.55).clamp(800.0, 1000.0);
            let back = 200.0;
            out[0] = shape(vec![
                p(x0, y0),
                p(x1, y0),
                p(x1, y1),
                p(x1 - seat, y1),
                p(x1 - seat, y0 + seat),
                p(x0, y0 + seat),
            ]);
            out.push(line(p(x0, y0 + back), p(x1 - back, y0 + back)));
            out.push(line(p(x1 - back, y0 + back), p(x1 - back, y1)));
            out.push(line(p(x1 - seat, y0 + seat), p(x1 - back, y0 + seat)));
        }
        "coffee-table" => out.push(shape(rounded(
            x0 + 40.0,
            y0 + 40.0,
            x1 - 40.0,
            y1 - 40.0,
            40.0,
        ))),
        "tv-stand" => {
            out.push(line(p(x0, y1 - 40.0), p(x1, y1 - 40.0)));
            out.push(shape(rect(-w * 0.4, y0 + 40.0, w * 0.4, y0 + 100.0)));
        }
        "bookcase" | "shelf" => {
            out.push(line(p(x0, y1 - 20.0), p(x1, y1 - 20.0)));
            out.push(line(p(x0, y0), p(x1, y1)));
        }
        "kitchen-base" => out.push(line(p(x0, y1 - 40.0), p(x1, y1 - 40.0))),
        "kitchen-wall" => {
            out[0].dashed = true;
            out.push(Mark {
                points: vec![p(x0, y0), p(x1, y1)],
                closed: false,
                dashed: true,
            });
        }
        "kitchen-sink" => {
            out.push(shape(rounded(
                x0 + 80.0,
                y0 + 100.0,
                x1 - 80.0,
                y1 - 60.0,
                70.0,
            )));
            out.push(shape(ellipse(p(0.0, y0 + 60.0), 25.0, 25.0)));
        }
        "stove" => {
            for (cx, cy, r) in [
                (-0.25, -0.22, 0.17),
                (0.25, -0.22, 0.13),
                (-0.25, 0.22, 0.13),
                (0.25, 0.22, 0.17),
            ] {
                out.push(shape(ellipse(p(cx * w, cy * d), r * w, r * w)));
            }
        }
        "fridge" | "washer" => {
            out.push(line(p(x0, y1 - 40.0), p(x1, y1 - 40.0)));
            if item == "washer" {
                out.push(shape(ellipse(p(0.0, 0.0), w * 0.33, w * 0.33)));
            } else {
                out.push(line(p(x0, y0), p(x1, y1 - 40.0)));
            }
        }
        "dining-table" | "desk" => {
            out.push(shape(rect(x0 + 30.0, y0 + 30.0, x1 - 30.0, y1 - 30.0)));
        }
        "round-table" => {
            out[0] = shape(ellipse(p(0.0, 0.0), hx, hy));
        }
        "chair" | "office-chair" => {
            if item == "office-chair" {
                out[0] = shape(ellipse(p(0.0, 0.0), hx, hy));
                out.push(shape(rounded(-hx * 0.8, y0, hx * 0.8, y0 + d * 0.2, 60.0)));
            } else {
                out.push(shape(rect(x0, y0, x1, y0 + d * 0.15)));
            }
        }
        "bathtub" => {
            out.push(shape(rounded(
                x0 + 70.0,
                y0 + 70.0,
                x1 - 70.0,
                y1 - 70.0,
                d * 0.35,
            )));
            out.push(shape(ellipse(p(x0 + 200.0, 0.0), 25.0, 25.0)));
        }
        "shower" => {
            out.push(line(p(x0, y0), p(x1, y1)));
            out.push(line(p(x1, y0), p(x0, y1)));
            out.push(shape(ellipse(p(0.0, 0.0), 40.0, 40.0)));
        }
        "toilet" => {
            out[0] = shape(rect(x0, y0, x1, y0 + d * 0.28));
            out.push(shape(ellipse(p(0.0, y0 + d * 0.62), hx * 0.9, d * 0.36)));
        }
        "washbasin" => {
            out.push(shape(ellipse(p(0.0, 0.05 * d), hx * 0.75, hy * 0.6)));
            out.push(shape(ellipse(p(0.0, y0 + 50.0), 20.0, 20.0)));
        }
        _ => {}
    }
    out
}

/// Деталь 3D-формы: брусок в локальных осях (мм) и цвет.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Part {
    /// Минимальный угол.
    pub lo: DVec3,
    /// Максимальный угол.
    pub hi: DVec3,
    /// Цвет `0xRRGGBB`.
    pub color: u32,
}

const WOOD: u32 = 0xB08A63;
const FABRIC: u32 = 0x8E9BA8;
const LINEN: u32 = 0xEDEAE3;
const WHITE: u32 = 0xF4F4F2;
const METAL: u32 = 0xC9CCCF;
const DARK: u32 = 0x4A4A48;

/// 3D-форма предмета из брусков; `h` — полная высота.
#[must_use]
pub fn parts(item: &str, w: f64, d: f64, h: f64) -> Vec<Part> {
    let (hx, hy) = (w / 2.0, d / 2.0);
    let b = |x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64, color: u32| Part {
        lo: DVec3::new(x0, y0, z0),
        hi: DVec3::new(x1, y1, z1),
        color,
    };
    let full = |color| vec![b(-hx, -hy, 0.0, hx, hy, h, color)];
    let table = |top: f64, color: u32| {
        let leg = 50.0;
        let mut v = vec![b(-hx, -hy, h - top, hx, hy, h, color)];
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let (cx, cy) = (sx * (hx - leg), sy * (hy - leg));
            v.push(b(
                cx - leg / 2.0,
                cy - leg / 2.0,
                0.0,
                cx + leg / 2.0,
                cy + leg / 2.0,
                h - top,
                color,
            ));
        }
        v
    };
    match item {
        "bed-double" | "bed-single" => vec![
            b(-hx, -hy, 0.0, hx, hy, h * 0.6, WOOD),
            b(
                -hx + 30.0,
                -hy + 60.0,
                h * 0.6,
                hx - 30.0,
                hy - 30.0,
                h,
                LINEN,
            ),
            b(-hx, -hy, 0.0, hx, -hy + 60.0, h * 2.0, WOOD),
        ],
        "sofa" | "armchair" => {
            let arm = (w * 0.1).clamp(120.0, 220.0);
            let back = (d * 0.22).clamp(150.0, 250.0);
            vec![
                b(-hx, -hy, 0.0, hx, hy, h * 0.5, FABRIC),
                b(-hx, -hy, 0.0, hx, -hy + back, h, FABRIC),
                b(-hx, -hy, 0.0, -hx + arm, hy, h * 0.7, FABRIC),
                b(hx - arm, -hy, 0.0, hx, hy, h * 0.7, FABRIC),
            ]
        }
        "sofa-corner" => {
            let seat = (d * 0.55).clamp(800.0, 1000.0);
            vec![
                b(-hx, -hy, 0.0, hx, -hy + seat, h * 0.5, FABRIC),
                b(hx - seat, -hy, 0.0, hx, hy, h * 0.5, FABRIC),
                b(-hx, -hy, 0.0, hx, -hy + 200.0, h, FABRIC),
                b(hx - 200.0, -hy, 0.0, hx, hy, h, FABRIC),
            ]
        }
        "dining-table" | "round-table" | "coffee-table" | "desk" => table(40.0, WOOD),
        "chair" | "office-chair" => {
            let seat = h * 0.5;
            let color = if item == "chair" { WOOD } else { DARK };
            let mut v = table(40.0, color);
            for p in &mut v {
                p.hi.z = p.hi.z.min(seat);
                if p.lo.z > 0.0 {
                    p.lo.z = seat - 40.0;
                }
            }
            v.push(b(-hx, -hy, seat, hx, -hy + 40.0, h, color));
            v
        }
        "fridge" | "stove" | "washer" => full(METAL),
        "kitchen-base" | "kitchen-sink" => vec![
            b(-hx, -hy, 0.0, hx, hy - 20.0, h - 40.0, WHITE),
            b(-hx, -hy, h - 40.0, hx, hy, h, WOOD),
        ],
        "bathtub" | "washbasin" | "toilet" | "shower" | "kitchen-wall" | "wardrobe"
        | "hallway-wardrobe" | "dresser" | "nightstand" | "shoe-cabinet" | "tv-stand" => {
            full(WHITE)
        }
        _ => full(WOOD),
    }
}

/// Своя модель: треугольники и контур сверху в единичном кубе
/// (x — ширина, y — глубина, лицевая сторона на +y, z — высота).
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// Треугольники, точки в [0, 1]³.
    pub triangles: Vec<[DVec3; 3]>,
    /// Выпуклая оболочка вида сверху в [0, 1]².
    pub hull: Vec<DVec2>,
    /// Размеры модели в файле, мм: ширина, глубина, высота.
    pub size: [f64; 3],
}

/// Читает GLB своей модели: ось Y файла — вверх, +Z — лицевая сторона, метры.
///
/// # Errors
///
/// Файл не читается как GLB или в нём нет треугольников.
pub fn read_model(bytes: &[u8]) -> Result<Model, String> {
    if !bytes.starts_with(b"glTF") {
        return Err("нужен файл GLB (glTF 2.0)".into());
    }
    let loaded = op_scan::io::load(bytes).map_err(|err| err.to_string())?;
    let to_mm = loaded.to_mm.unwrap_or(1000.0);
    let mesh = loaded.mesh;
    if mesh.triangles.is_empty() {
        return Err("в модели нет треугольников".into());
    }
    let points: Vec<DVec3> = mesh
        .positions
        .iter()
        .map(|p| DVec3::new(p.x, p.z, p.y) * to_mm)
        .collect();
    let (lo, hi) = points.iter().fold(
        (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    let size = (hi - lo).max(DVec3::splat(1.0));
    let unit = |p: DVec3| (p - lo) / size;
    let triangles = mesh
        .triangles
        .iter()
        .filter_map(|t| {
            Some([
                unit(*points.get(t[0] as usize)?),
                unit(*points.get(t[1] as usize)?),
                unit(*points.get(t[2] as usize)?),
            ])
        })
        .collect();
    let flat: Vec<DVec2> = points.iter().map(|p| unit(*p).truncate()).collect();
    Ok(Model {
        triangles,
        hull: op_geom::hull(&flat),
        size: [size.x, size.y, size.z],
    })
}

/// Точка в локальных осях предмета → точка плана.
#[must_use]
pub fn to_plan(f: &Furniture, local: DVec2) -> DVec2 {
    let x = if f.mirrored { -local.x } else { local.x };
    let (sin, cos) = f.rotation.to_radians().sin_cos();
    DVec2::new(f.x, f.y) + DVec2::new(x * cos - local.y * sin, x * sin + local.y * cos)
}

/// Углы габарита на плане.
#[must_use]
pub fn corners(f: &Furniture) -> Vec<DVec2> {
    rect(-f.width / 2.0, -f.depth / 2.0, f.width / 2.0, f.depth / 2.0)
        .into_iter()
        .map(|p| to_plan(f, p))
        .collect()
}

/// Обозначение предмета на плане: линии в координатах плана.
#[must_use]
pub fn plan_marks(f: &Furniture, model: Option<&Model>) -> Vec<Mark> {
    let marks = match model {
        Some(m) if f.model.is_some() => {
            let hull = m
                .hull
                .iter()
                .map(|p| DVec2::new((p.x - 0.5) * f.width, (p.y - 0.5) * f.depth))
                .collect();
            vec![
                shape(rect(
                    -f.width / 2.0,
                    -f.depth / 2.0,
                    f.width / 2.0,
                    f.depth / 2.0,
                )),
                shape(hull),
            ]
        }
        _ => symbol(&f.item, f.width, f.depth),
    };
    let dashed_above = f.elevation > 1000.0;
    marks
        .into_iter()
        .map(|m| Mark {
            points: m.points.iter().map(|p| to_plan(f, *p)).collect(),
            dashed: m.dashed || dashed_above,
            ..m
        })
        .collect()
}

/// Попадает ли точка в габарит предмета.
#[must_use]
pub fn contains(f: &Furniture, p: DVec2, pad: f64) -> bool {
    let d = p - DVec2::new(f.x, f.y);
    let (sin, cos) = f.rotation.to_radians().sin_cos();
    let local = DVec2::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos);
    local.x.abs() <= f.width / 2.0 + pad && local.y.abs() <= f.depth / 2.0 + pad
}

/// Поворот, при котором лицевая сторона предмета смотрит вдоль `facing` (план, Y вниз).
#[must_use]
pub fn rotation_facing(facing: DVec2) -> f64 {
    // Лицевая сторона — локальная +Y; при повороте θ она уходит в (−sin θ, cos θ).
    (-facing.x).atan2(facing.y).to_degrees().rem_euclid(360.0)
}

/// Привязка к стене: если спинка предмета ближе `radius` к грани стены, предмет
/// встаёт спинкой к грани, лицом в помещение. Возвращает центр и поворот.
#[must_use]
pub fn snap_to_wall(
    doc: &Document,
    derived: &Derived,
    center: DVec2,
    depth: f64,
    radius: f64,
) -> Option<(DVec2, f64)> {
    let mut best: Option<(f64, DVec2, f64)> = None;
    for w in doc
        .walls
        .iter()
        .filter(|w| derived.visible.contains(&w.layer))
    {
        let Some((a, b)) = derived.wall_axis(w) else {
            continue;
        };
        let len = a.distance(b);
        if len < 1.0 {
            continue;
        }
        let dir = (b - a) / len;
        let n = left_normal(dir);
        let (left, right) = w.sides();
        for (origin, out) in [(a + n * left, n), (a - n * right, -n)] {
            let along = (center - origin).dot(dir);
            if !(0.0..=len).contains(&along) {
                continue;
            }
            // Центр перед гранью на половину глубины.
            let off = (center - origin).dot(out);
            let error = (off - depth / 2.0).abs();
            if off > 0.0 && error <= radius && best.is_none_or(|(e, ..)| error < e) {
                let at = origin + dir * along + out * (depth / 2.0);
                best = Some((error, at, rotation_facing(out)));
            }
        }
    }
    best.map(|(_, at, rotation)| (at, rotation))
}

/// Позиция каталога для UI: иконка — обозначение в квадрате 100 × 100 (путь SVG).
#[must_use]
pub fn catalog_entries() -> Vec<CatalogEntry> {
    CATALOG
        .iter()
        .map(|i| {
            let [w, d, h] = i.size;
            let k = 90.0 / w.max(d);
            let path = symbol(i.id, w, d)
                .iter()
                .map(|m| {
                    let mut s = String::new();
                    for (j, p) in m.points.iter().enumerate() {
                        let q = DVec2::splat(50.0) + *p * k;
                        s.push_str(&format!(
                            "{}{:.1} {:.1}",
                            if j == 0 { "M" } else { "L" },
                            q.x,
                            q.y
                        ));
                    }
                    if m.closed {
                        s.push('Z');
                    }
                    s
                })
                .collect::<String>();
            CatalogEntry {
                id: i.id.to_owned(),
                name: i.name.to_owned(),
                category: i.category.name().to_owned(),
                width: w,
                depth: d,
                height: h,
                elevation: i.elevation,
                keywords: i.keywords.to_owned(),
                icon: path,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bed(rotation: f64, mirrored: bool) -> Furniture {
        Furniture {
            id: "f".into(),
            layer: op_model::LayerKind::Furniture.id().into(),
            variant: None,
            item: "bed-double".into(),
            name: "Кровать".into(),
            x: 1000.0,
            y: 2000.0,
            rotation,
            mirrored,
            width: 1600.0,
            depth: 2000.0,
            height: 450.0,
            elevation: 0.0,
            model: None,
        }
    }

    #[test]
    fn catalog_items_are_unique_and_have_symbols() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|i| i.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        for i in CATALOG {
            let [w, d, h] = i.size;
            assert!(!symbol(i.id, w, d).is_empty(), "{}", i.id);
            assert!(!parts(i.id, w, d, h).is_empty());
        }
        assert!(catalog_entries().iter().all(|e| e.icon.starts_with('M')));
    }

    #[test]
    fn rotation_turns_clockwise_and_mirror_flips_width() {
        // Поворот на 90° по часовой: правый край (+X) уходит вниз (+Y плана).
        let f = bed(90.0, false);
        let p = to_plan(&f, DVec2::new(800.0, 0.0));
        assert!((p - DVec2::new(1000.0, 2800.0)).length() < 1e-9, "{p}");
        let m = bed(0.0, true);
        assert!((to_plan(&m, DVec2::new(800.0, 0.0)) - DVec2::new(200.0, 2000.0)).length() < 1e-9);
        assert!(
            contains(&f, DVec2::new(1900.0, 2000.0), 0.0),
            "глубина повёрнута вдоль X"
        );
        assert!(!contains(&f, DVec2::new(1000.0, 3000.0), 0.0));
        // Лицом вниз листа — поворот 0, лицом вправо — 270.
        assert!(rotation_facing(DVec2::Y).abs() < 1e-9);
        assert!((rotation_facing(DVec2::X) - 270.0).abs() < 1e-9);
    }
}
