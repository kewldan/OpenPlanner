//! Рендер без окна: кадр в текстуру и обратно в память (нативные бэкенды).
//!
//! Нужен для проверки шейдеров и конвейеров на настоящем GPU-бэкенде
//! платформы (Metal, D3D12, Vulkan) — тот же код, что в WASM работает через
//! WebGPU, — и для будущего оффлайн-рендера.

use crate::{
    Camera2D, RenderError,
    gpu::{GpuScene, Pipelines},
    gpu3d::Pipelines3D,
    scene::Scene,
    three::{Camera3D, Scene3D},
};

/// Кадр в памяти: RGBA8, строки подряд.
#[derive(Debug, Clone)]
pub struct Image {
    /// Ширина, пиксели.
    pub width: u32,
    /// Высота, пиксели.
    pub height: u32,
    /// Пиксели RGBA8 (sRGB).
    pub pixels: Vec<u8>,
    /// Адаптер, на котором отрисован кадр, например `Metal: Apple Paravirtual device`.
    pub adapter: String,
}

impl Image {
    /// Цвет пикселя.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }
}

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Изображение для сцены: ключ, ширина, высота, пиксели RGBA8.
pub type SceneImage<'a> = (&'a str, u32, u32, &'a [u8]);

/// Рисует сетку и сцену плана в текстуру и возвращает кадр.
///
/// # Errors
///
/// [`RenderError::Adapter`], если в системе нет подходящего GPU-адаптера.
pub fn render_plan_image(
    width: u32,
    height: u32,
    camera: &Camera2D,
    scene: &Scene,
    images: &[SceneImage<'_>],
) -> Result<Image, RenderError> {
    pollster::block_on(render(width, height, |device, queue, encoder, view| {
        let mut pipelines = Pipelines::new(device, queue, FORMAT);
        for (key, w, h, rgba) in images {
            pipelines.set_image(device, queue, key, *w, *h, rgba);
        }
        let mut gpu_scene = GpuScene::new(device);
        gpu_scene.upload(device, queue, scene);
        pipelines.prepare(queue, camera, (width, height), 1.0, false);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("plan"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pipelines.draw(&mut pass, &[&gpu_scene]);
    }))
}

/// Рисует 3D-сцену (с картой теней, если есть солнце) и возвращает кадр.
///
/// # Errors
///
/// [`RenderError::Adapter`], если в системе нет подходящего GPU-адаптера.
pub fn render_3d_image(
    width: u32,
    height: u32,
    camera: &Camera3D,
    scene: &Scene3D,
) -> Result<Image, RenderError> {
    pollster::block_on(render(width, height, |device, queue, encoder, view| {
        let mut three = Pipelines3D::new(device, FORMAT);
        three.upload(device, queue, scene);
        three.render(
            (device, queue),
            encoder,
            view,
            (width, height),
            camera,
            false,
        );
    }))
}

/// Создаёт устройство и текстуру кадра, выполняет `draw` и читает кадр обратно.
async fn render(
    width: u32,
    height: u32,
    draw: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder, &wgpu::TextureView),
) -> Result<Image, RenderError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .map_err(|err| RenderError::Adapter(err.to_string()))?;
    let info = adapter.get_info();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("offscreen"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|err| RenderError::Device(err.to_string()))?;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("frame"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Строки буфера выравниваются по 256 байт.
    let row = width * 4;
    let padded_row =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("frame"),
    });
    draw(&device, &queue, &mut encoder, &view);
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);

    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|err| RenderError::Device(err.to_string()))?;
    receiver
        .recv()
        .map_err(|err| RenderError::Device(err.to_string()))?
        .map_err(|err| RenderError::Device(err.to_string()))?;

    let mapped = slice
        .get_mapped_range()
        .map_err(|err| RenderError::Device(err.to_string()))?;
    let mut pixels = Vec::with_capacity((row * height) as usize);
    for line in mapped.chunks(padded_row as usize) {
        pixels.extend_from_slice(&line[..row as usize]);
    }
    drop(mapped);
    buffer.unmap();

    Ok(Image {
        width,
        height,
        pixels,
        adapter: format!("{:?}: {}", info.backend, info.name),
    })
}

#[cfg(test)]
mod tests {
    use glam::DVec2;

    use super::*;

    /// Без GPU тест пропускается, если не задан `OPENPLANNER_REQUIRE_GPU`
    /// (в CI задан: там рендер обязан пройти на бэкенде платформы).
    fn render_or_skip(camera: &Camera2D) -> Option<Image> {
        render_scene_or_skip(camera, &Scene::new(), &[])
    }

    fn render_scene_or_skip(
        camera: &Camera2D,
        scene: &Scene,
        images: &[SceneImage<'_>],
    ) -> Option<Image> {
        match render_plan_image(800, 600, camera, scene, images) {
            Ok(image) => Some(image),
            Err(err) if std::env::var_os("OPENPLANNER_REQUIRE_GPU").is_none() => {
                eprintln!("пропуск: нет GPU-адаптера ({err})");
                None
            }
            Err(err) => panic!("рендер не удался: {err}"),
        }
    }

    #[test]
    fn grid_frame_has_paper_and_lines() {
        // Масштаб 0,1 px/мм: метровые линии через 100 px, по ~1 px толщиной.
        let camera = Camera2D {
            center: DVec2::new(0.0, 0.0),
            px_per_mm: 0.1,
        };
        let Some(image) = render_or_skip(&camera) else {
            return;
        };
        println!("адаптер: {}", image.adapter);

        let total = image.width * image.height;
        let mut paper = 0;
        let mut lines = 0;
        for y in 0..image.height {
            for x in 0..image.width {
                let [r, g, b, _] = image.pixel(x, y);
                if r >= 0xf8 && g >= 0xf8 && b >= 0xf5 {
                    paper += 1;
                } else if (0xb0..0xf2).contains(&r) && (0xb0..0xf2).contains(&g) {
                    lines += 1;
                }
            }
        }
        assert!(
            paper * 2 > total,
            "больше половины кадра — цвет листа: {paper}/{total}"
        );
        assert!(lines * 100 > total, "видны линии сетки: {lines}/{total}");

        // Центр кадра — точка (0, 0) плана, через неё проходят метровые линии.
        let [r, ..] = image.pixel(image.width / 2, 10);
        assert!(
            r < 0xf2,
            "вертикальная линия x = 0 в центре кадра, цвет {r:#x}"
        );
    }

    #[test]
    fn scene_primitives_are_drawn() {
        use crate::{
            scene::{Align, Color, ImageQuad, TextStyle},
            text::Font,
        };
        // 1 px = 10 мм, центр кадра (400, 300) — точка (0, 0).
        let camera = Camera2D {
            center: DVec2::ZERO,
            px_per_mm: 0.1,
        };
        let mut scene = Scene::new();
        // Подложка: зелёное изображение 2×2 в левом верхнем углу.
        scene.image(ImageQuad {
            key: "green".into(),
            origin: DVec2::new(-3900.0, -2900.0),
            size: DVec2::new(1000.0, 1000.0),
            opacity: 1.0,
        });
        // Стена: чёрный прямоугольник с отверстием.
        let outer = [
            DVec2::new(-1000.0, -1000.0),
            DVec2::new(1000.0, -1000.0),
            DVec2::new(1000.0, 1000.0),
            DVec2::new(-1000.0, 1000.0),
        ];
        let hole = [
            DVec2::new(-800.0, -800.0),
            DVec2::new(800.0, -800.0),
            DVec2::new(800.0, 800.0),
            DVec2::new(-800.0, 800.0),
        ];
        scene.fill(&[&outer, &hole], Color::hex(0x202020, 1.0));
        // Красная линия 4 px справа.
        scene.line(
            DVec2::new(2000.0, -1000.0),
            DVec2::new(2000.0, 1000.0),
            4.0,
            Color::hex(0xFF0000, 1.0),
        );
        // Синий маркер.
        scene.dot(DVec2::new(-2000.0, 0.0), 12.0, Color::hex(0x0000FF, 1.0));
        // Текст крупно в центре отверстия.
        let font = Font::shared().unwrap();
        scene.text(
            font,
            DVec2::ZERO,
            "ШШШ",
            TextStyle {
                size: 600.0,
                screen: false,
                align: Align::Center,
                angle: 0.0,
                color: Color::hex(0x000000, 1.0),
                halo: 0.0,
            },
        );
        let green = [0u8, 200, 0, 255].repeat(4);
        let Some(image) = render_scene_or_skip(&camera, &scene, &[("green", 2, 2, &green)]) else {
            return;
        };
        let dark = |x, y| {
            let [r, g, b, _] = image.pixel(x, y);
            r < 0x40 && g < 0x40 && b < 0x40
        };
        // Тело стены (−900 мм по X → 310 px).
        assert!(dark(310, 300), "стена закрашена");
        // Отверстие у края — светлое.
        assert!(!dark(330, 260), "отверстие не закрашено");
        // Текст «ШШШ» по центру — есть тёмные пиксели в ряду базовой линии.
        let text_dark = (360..440).filter(|&x| dark(x, 298)).count();
        assert!(text_dark > 5, "текст нарисован: {text_dark}");
        let [r, g, b, _] = image.pixel(600, 300);
        assert!(
            r > 0xd0 && g < 0x40 && b < 0x40,
            "линия красная: {r} {g} {b}"
        );
        let [r, g, b, _] = image.pixel(200, 300);
        assert!(b > 0xd0 && r < 0x40, "маркер синий: {r} {g} {b}");
        let [r, g, b, _] = image.pixel(15, 15);
        assert!(
            g > 0xb0 && r < 0x40 && b < 0x40,
            "подложка зелёная: {r} {g} {b}"
        );
    }

    #[test]
    fn sun_casts_shadow_in_3d() {
        use glam::DVec3;

        use crate::{
            scene::Color,
            three::{Mesh, Sun},
        };
        let square = |lo: f64, hi: f64| {
            vec![
                DVec2::new(lo, lo),
                DVec2::new(hi, lo),
                DVec2::new(hi, hi),
                DVec2::new(lo, hi),
            ]
        };
        let mut opaque = Mesh::default();
        let white = Color([0.9, 0.9, 0.9, 1.0]);
        opaque.cap(&[square(-3000.0, 3000.0)], 0.0, true, white);
        opaque.block(
            [
                DVec2::new(-200.0, -1500.0),
                DVec2::new(200.0, -1500.0),
                DVec2::new(200.0, 1500.0),
                DVec2::new(-200.0, 1500.0),
            ],
            0.0,
            2500.0,
            white,
        );
        let scene = Scene3D {
            opaque,
            glass: Mesh::default(),
            sun: Some(Sun {
                direction: DVec3::new(1.0, 0.0, 1.0).normalize(),
                intensity: 1.0,
            }),
        };
        let camera = Camera3D {
            eye: DVec3::new(0.0, -9000.0, 9000.0),
            target: DVec3::ZERO,
            fov_y: 0.9,
        };
        let image = match render_3d_image(800, 600, &camera, &scene) {
            Ok(image) => image,
            Err(err) if std::env::var_os("OPENPLANNER_REQUIRE_GPU").is_none() => {
                eprintln!("пропуск: нет GPU-адаптера ({err})");
                return;
            }
            Err(err) => panic!("рендер не удался: {err}"),
        };
        let m = camera.view_proj(800.0 / 600.0);
        let at = |p: DVec3| {
            let n = m.project_point3(p);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // внутри кадра
            let px = ((n.x * 0.5 + 0.5) * 800.0) as u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let py = ((0.5 - n.y * 0.5) * 600.0) as u32;
            let [r, g, b, _] = image.pixel(px, py);
            u32::from(r) + u32::from(g) + u32::from(b)
        };
        let shadow = at(DVec3::new(-1500.0, 0.0, 0.0));
        let lit = at(DVec3::new(1500.0, 0.0, 0.0));
        assert!(
            lit > shadow + 90,
            "тень за стенкой: освещено {lit}, в тени {shadow}"
        );

        // Солнце с юга (−Y): тень ложится к северу (+Y), а не зеркально.
        let rotated = Scene3D {
            opaque: {
                let mut m = Mesh::default();
                m.cap(&[square(-3000.0, 3000.0)], 0.0, true, white);
                m.block(
                    [
                        DVec2::new(-1500.0, -200.0),
                        DVec2::new(1500.0, -200.0),
                        DVec2::new(1500.0, 200.0),
                        DVec2::new(-1500.0, 200.0),
                    ],
                    0.0,
                    2500.0,
                    white,
                );
                m
            },
            glass: Mesh::default(),
            sun: Some(Sun {
                direction: DVec3::new(0.0, -1.0, 1.0).normalize(),
                intensity: 1.0,
            }),
        };
        let top = Camera3D {
            eye: DVec3::new(0.0, -1.0, 12_000.0),
            target: DVec3::ZERO,
            fov_y: 0.9,
        };
        let Ok(image2) = render_3d_image(800, 600, &top, &rotated) else {
            return;
        };
        let m2 = top.view_proj(800.0 / 600.0);
        let at2 = |p: DVec3| {
            let n = m2.project_point3(p);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // внутри кадра
            let px = ((n.x * 0.5 + 0.5) * 800.0) as u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let py = ((0.5 - n.y * 0.5) * 600.0) as u32;
            let [r, g, b, _] = image2.pixel(px, py);
            u32::from(r) + u32::from(g) + u32::from(b)
        };
        let north = at2(DVec3::new(0.0, 1500.0, 0.0));
        let south = at2(DVec3::new(0.0, -1500.0, 0.0));
        assert!(
            south > north + 90,
            "тень к северу: юг {south}, север {north}"
        );
        // Угол кадра — небо.
        let [r, g, b, _] = image.pixel(2, 2);
        assert!(b >= r && (0xe0..=0xf8).contains(&g), "небо: {r} {g} {b}");
    }
}
