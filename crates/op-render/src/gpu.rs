//! GPU-часть рендерера: конвейеры, буферы сцены, текстуры изображений.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};

use crate::{
    Camera2D,
    scene::{FillVertexData, GlyphInstance, LineInstance, Scene},
    text::Font,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    viewport: [f32; 2],
    center: [f32; 2],
    scale: f32,
    srgb_output: f32,
    dpr: f32,
    pad: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ImageVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    opacity: f32,
    pad: f32,
}

/// Буфер, который растёт по мере надобности.
#[derive(Debug)]
struct GrowBuffer {
    buffer: wgpu::Buffer,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl GrowBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let usage = usage | wgpu::BufferUsages::COPY_DST;
        Self {
            buffer: Self::create(device, label, usage, 1024),
            usage,
            label,
        }
    }

    fn create(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        size: u64,
    ) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    }

    fn write(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        let len = data.len() as u64;
        if len > self.buffer.size() {
            self.buffer = Self::create(
                device,
                self.label,
                self.usage,
                len.next_power_of_two().max(1024),
            );
        }
        if !data.is_empty() {
            // Длина записи должна быть кратна 4 — данные из f32/u32 всегда кратны.
            queue.write_buffer(&self.buffer, 0, data);
        }
    }
}

/// Сцена в буферах GPU.
#[derive(Debug)]
pub(crate) struct GpuScene {
    fill_vertices: GrowBuffer,
    fill_indices: GrowBuffer,
    lines: GrowBuffer,
    glyphs: GrowBuffer,
    images: GrowBuffer,
    index_count: u32,
    line_count: u32,
    glyph_count: u32,
    /// Ключи изображений по порядку (по 6 вершин на изображение).
    image_keys: Vec<String>,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl GpuScene {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        Self {
            fill_vertices: GrowBuffer::new(device, "fill vertices", wgpu::BufferUsages::VERTEX),
            fill_indices: GrowBuffer::new(device, "fill indices", wgpu::BufferUsages::INDEX),
            lines: GrowBuffer::new(device, "lines", wgpu::BufferUsages::VERTEX),
            glyphs: GrowBuffer::new(device, "glyphs", wgpu::BufferUsages::VERTEX),
            images: GrowBuffer::new(device, "images", wgpu::BufferUsages::VERTEX),
            index_count: 0,
            line_count: 0,
            glyph_count: 0,
            image_keys: Vec::new(),
        }
    }

    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) {
        self.fill_vertices
            .write(device, queue, bytemuck::cast_slice(&scene.fill_vertices));
        self.fill_indices
            .write(device, queue, bytemuck::cast_slice(&scene.fill_indices));
        self.lines
            .write(device, queue, bytemuck::cast_slice(&scene.lines));
        self.glyphs
            .write(device, queue, bytemuck::cast_slice(&scene.glyphs));
        let mut vertices = Vec::with_capacity(scene.images.len() * 6);
        for image in &scene.images {
            let o = image.origin;
            let s = image.size;
            #[allow(clippy::cast_possible_truncation)] // f32 достаточно для GPU
            let v = |x: f64, y: f64, u: f32, w: f32| ImageVertex {
                pos: [x as f32, y as f32],
                uv: [u, w],
                opacity: image.opacity,
                pad: 0.0,
            };
            let (x0, y0, x1, y1) = (o.x, o.y, o.x + s.x, o.y + s.y);
            vertices.extend([
                v(x0, y0, 0.0, 0.0),
                v(x1, y0, 1.0, 0.0),
                v(x1, y1, 1.0, 1.0),
                v(x0, y0, 0.0, 0.0),
                v(x1, y1, 1.0, 1.0),
                v(x0, y1, 0.0, 1.0),
            ]);
        }
        self.images
            .write(device, queue, bytemuck::cast_slice(&vertices));
        self.index_count = count(scene.fill_indices.len());
        self.line_count = count(scene.lines.len());
        self.glyph_count = count(scene.glyphs.len());
        self.image_keys = scene.images.iter().map(|i| i.key.clone()).collect();
    }
}

/// Текстура изображения с группой привязки.
#[derive(Debug)]
struct GpuImage {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// Все конвейеры и общие ресурсы.
#[derive(Debug)]
pub(crate) struct Pipelines {
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    grid: wgpu::RenderPipeline,
    fill: wgpu::RenderPipeline,
    line: wgpu::RenderPipeline,
    glyph: wgpu::RenderPipeline,
    image: wgpu::RenderPipeline,
    atlas: Option<GpuImage>,
    images: HashMap<String, GpuImage>,
    max_texture: u32,
}

const BLEND: wgpu::BlendState = wgpu::BlendState::ALPHA_BLENDING;

impl Pipelines {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let plan = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("plan"),
            source: wgpu::ShaderSource::Wgsl(include_str!("plan.wgsl").into()),
        });
        let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("grid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grid.wgsl").into()),
        });
        let plain_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("plain"),
            bind_group_layouts: &[Some(&globals_layout)],
            ..Default::default()
        });
        let textured_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("textured"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            ..Default::default()
        });

        let make = |label: &str,
                    module: &wgpu::ShaderModule,
                    vs: &str,
                    fs: &str,
                    layout: &wgpu::PipelineLayout,
                    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                    blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some(vs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some(fs),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        let grid = make(
            "grid",
            &grid_shader,
            "vs_main",
            "fs_main",
            &plain_layout,
            &[],
            None,
        );
        let fill = make(
            "fill",
            &plan,
            "fill_vs",
            "fill_fs",
            &plain_layout,
            &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<FillVertexData>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            })],
            Some(BLEND),
        );
        let line = make(
            "line",
            &plan,
            "line_vs",
            "line_fs",
            &plain_layout,
            &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<LineInstance>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    attr(0, 0, wgpu::VertexFormat::Float32x2),
                    attr(1, 8, wgpu::VertexFormat::Float32x2),
                    attr(2, 16, wgpu::VertexFormat::Float32),
                    attr(3, 20, wgpu::VertexFormat::Float32),
                    attr(4, 24, wgpu::VertexFormat::Float32),
                    attr(5, 32, wgpu::VertexFormat::Float32x4),
                ],
            })],
            Some(BLEND),
        );
        let glyph = make(
            "glyph",
            &plan,
            "glyph_vs",
            "glyph_fs",
            &textured_layout,
            &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GlyphInstance>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &[
                    attr(0, 0, wgpu::VertexFormat::Float32x2),
                    attr(1, 8, wgpu::VertexFormat::Float32),
                    attr(2, 12, wgpu::VertexFormat::Float32),
                    attr(3, 16, wgpu::VertexFormat::Float32x2),
                    attr(4, 24, wgpu::VertexFormat::Float32x2),
                    attr(5, 32, wgpu::VertexFormat::Float32x2),
                    attr(6, 40, wgpu::VertexFormat::Float32x2),
                    attr(7, 48, wgpu::VertexFormat::Float32),
                    attr(8, 52, wgpu::VertexFormat::Float32),
                    attr(9, 64, wgpu::VertexFormat::Float32x4),
                ],
            })],
            Some(BLEND),
        );
        let image = make(
            "image",
            &plan,
            "image_vs",
            "image_fs",
            &textured_layout,
            &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<ImageVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32],
            })],
            Some(BLEND),
        );

        let mut pipelines = Self {
            globals,
            globals_group,
            texture_layout,
            sampler,
            grid,
            fill,
            line,
            glyph,
            image,
            atlas: None,
            images: HashMap::new(),
            max_texture: device.limits().max_texture_dimension_2d,
        };
        if let Some(font) = Font::shared() {
            pipelines.atlas = Some(pipelines.texture(
                device,
                queue,
                font.atlas_width,
                font.atlas_height,
                wgpu::TextureFormat::R8Unorm,
                &font.atlas,
            ));
        }
        pipelines
    }

    fn texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        data: &[u8],
    ) -> GpuImage {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("image"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let bytes_per_pixel = format.block_copy_size(None).unwrap_or(4);
        queue.write_texture(
            texture.as_image_copy(),
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * bytes_per_pixel),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        GpuImage {
            _texture: texture,
            bind_group,
        }
    }

    /// Загружает RGBA-изображение. Слишком большое уменьшается до лимита текстуры.
    pub(crate) fn set_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        if width == 0 || height == 0 || rgba.len() < (width as usize) * (height as usize) * 4 {
            return;
        }
        let (w, h, data) = fit(width, height, rgba, self.max_texture.min(8192));
        let image = self.texture(device, queue, w, h, wgpu::TextureFormat::Rgba8Unorm, &data);
        self.images.insert(key.to_owned(), image);
    }

    /// Убирает изображение.
    pub(crate) fn remove_image(&mut self, key: &str) {
        self.images.remove(key);
    }

    /// Загружено ли изображение.
    pub(crate) fn has_image(&self, key: &str) -> bool {
        self.images.contains_key(key)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // f32 достаточно для шейдера
    pub(crate) fn prepare(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera2D,
        size_px: (u32, u32),
        device_pixel_ratio: f64,
        srgb_output: bool,
    ) {
        let globals = Globals {
            viewport: [size_px.0 as f32, size_px.1 as f32],
            center: [camera.center.x as f32, camera.center.y as f32],
            scale: (camera.px_per_mm * device_pixel_ratio) as f32,
            srgb_output: if srgb_output { 1.0 } else { 0.0 },
            dpr: device_pixel_ratio as f32,
            pad: 0.0,
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
    }

    /// Рисует сетку и сцены по порядку.
    pub(crate) fn draw(&self, pass: &mut wgpu::RenderPass<'_>, scenes: &[&GpuScene]) {
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_pipeline(&self.grid);
        pass.draw(0..3, 0..1);
        for scene in scenes {
            if !scene.image_keys.is_empty() {
                pass.set_pipeline(&self.image);
                pass.set_vertex_buffer(0, scene.images.buffer.slice(..));
                for (i, key) in scene.image_keys.iter().enumerate() {
                    if let Some(image) = self.images.get(key) {
                        pass.set_bind_group(1, &image.bind_group, &[]);
                        let start = count(i * 6);
                        pass.draw(start..start + 6, 0..1);
                    }
                }
            }
            if scene.index_count > 0 {
                pass.set_pipeline(&self.fill);
                pass.set_vertex_buffer(0, scene.fill_vertices.buffer.slice(..));
                pass.set_index_buffer(
                    scene.fill_indices.buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..scene.index_count, 0, 0..1);
            }
            if scene.line_count > 0 {
                pass.set_pipeline(&self.line);
                pass.set_vertex_buffer(0, scene.lines.buffer.slice(..));
                pass.draw(0..6, 0..scene.line_count);
            }
            if scene.glyph_count > 0
                && let Some(atlas) = &self.atlas
            {
                pass.set_pipeline(&self.glyph);
                pass.set_bind_group(1, &atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, scene.glyphs.buffer.slice(..));
                pass.draw(0..6, 0..scene.glyph_count);
            }
        }
    }
}

fn attr(location: u32, offset: u64, format: wgpu::VertexFormat) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        format,
        offset,
        shader_location: location,
    }
}

/// Уменьшает изображение (ближайший пиксель), чтобы стороны не превышали `max`.
fn fit(width: u32, height: u32, rgba: &[u8], max: u32) -> (u32, u32, Vec<u8>) {
    if width <= max && height <= max {
        return (
            width,
            height,
            rgba[..(width * height * 4) as usize].to_vec(),
        );
    }
    let k = f64::from(width.max(height)) / f64::from(max);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // результат ≤ max
    let (w, h) = (
        ((f64::from(width) / k).floor() as u32).max(1),
        ((f64::from(height) / k).floor() as u32).max(1),
    );
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // внутри исходного размера
        let sy = ((f64::from(y) * k) as u32).min(height - 1);
        for x in 0..w {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let sx = ((f64::from(x) * k) as u32).min(width - 1);
            let i = ((sy * width + sx) * 4) as usize;
            out.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    (w, h, out)
}
