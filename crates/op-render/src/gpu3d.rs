//! GPU-часть 3D-вида: сетки в буферах, карта теней, конвейеры.

use bytemuck::{Pod, Zeroable};
use glam::{DMat4, DVec3};

use crate::three::{Camera3D, Mesh, Scene3D, Vertex3, sun_view_proj};

/// Размер карты теней, пиксели.
const SHADOW_SIZE: u32 = 2048;
/// Размер текселя карты теней в текстурных координатах.
const SHADOW_TEXEL: f32 = 1.0 / 2048.0;
/// Формат глубины кадра и карты теней.
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Цвет неба за моделью.
pub(crate) const SKY: wgpu::Color = wgpu::Color {
    r: 0.914,
    g: 0.933,
    b: 0.953,
    a: 1.0,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals3 {
    view_proj: [[f32; 4]; 4],
    sun_view_proj: [[f32; 4]; 4],
    sun: [f32; 4],
    params: [f32; 4],
}

#[allow(clippy::cast_possible_truncation)] // f32 достаточно для шейдера
fn mat(m: DMat4) -> [[f32; 4]; 4] {
    m.as_mat4().to_cols_array_2d()
}

/// Сетка в буферах GPU.
#[derive(Debug)]
struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

impl GpuMesh {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, mesh: &Mesh) -> Option<Self> {
        if mesh.is_empty() {
            return None;
        }
        let buffer = |label: &str, data: &[u8], usage: wgpu::BufferUsages| {
            let size = (data.len() as u64).next_multiple_of(4).max(4);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, data);
            buffer
        };
        Some(Self {
            vertices: buffer(
                "mesh vertices",
                bytemuck::cast_slice(&mesh.vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            indices: buffer(
                "mesh indices",
                bytemuck::cast_slice(&mesh.indices),
                wgpu::BufferUsages::INDEX,
            ),
            count: u32::try_from(mesh.indices.len()).unwrap_or(u32::MAX),
        })
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.count, 0, 0..1);
    }
}

/// Конвейеры, карта теней, глубина кадра и загруженная сцена.
#[derive(Debug)]
pub(crate) struct Pipelines3D {
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    shadow_group: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    shadow: wgpu::RenderPipeline,
    opaque: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    depth: Option<(u32, u32, wgpu::TextureView)>,
    scene_opaque: Option<GpuMesh>,
    scene_glass: Option<GpuMesh>,
    sun: Option<(DVec3, f64)>,
    bounds: Option<(DVec3, DVec3)>,
}

impl Pipelines3D {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals 3d"),
            size: size_of::<Globals3>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals 3d"),
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
            label: Some("globals 3d"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow map"),
            size: wgpu::Extent3d {
                width: SHADOW_SIZE,
                height: SHADOW_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow map"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("three"),
            source: wgpu::ShaderSource::Wgsl(include_str!("three.wgsl").into()),
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow"),
                bind_group_layouts: &[Some(&globals_layout)],
                ..Default::default()
            });
        let mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh"),
            bind_group_layouts: &[Some(&globals_layout), Some(&shadow_layout)],
            ..Default::default()
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex3>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4],
        };

        let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("shadow_vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(vertex_layout.clone())],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let mesh_pipeline = |label: &str, glass: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&mesh_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("mesh_vs"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(vertex_layout.clone())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("mesh_fs"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: glass.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: (!glass).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(!glass),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let opaque = mesh_pipeline("mesh", false);
        let glass = mesh_pipeline("glass", true);

        Self {
            globals,
            globals_group,
            shadow_group,
            shadow_view,
            shadow,
            opaque,
            glass,
            depth: None,
            scene_opaque: None,
            scene_glass: None,
            sun: None,
            bounds: None,
        }
    }

    /// Загружает сцену.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene3D) {
        self.scene_opaque = GpuMesh::new(device, queue, &scene.opaque);
        self.scene_glass = GpuMesh::new(device, queue, &scene.glass);
        self.sun = scene.sun.map(|s| (s.direction, s.intensity));
        self.bounds = scene.opaque.bounds();
    }

    /// Буфер глубины размера кадра.
    fn depth_view(&mut self, device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        if let Some((w, h, view)) = &self.depth
            && (*w, *h) == (width, height)
        {
            return view.clone();
        }
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.depth = Some((width, height, view.clone()));
            view
        }
    }

    /// Рисует кадр в `target` размера `width × height`: карта теней, затем сцена.
    pub(crate) fn render(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        camera: &Camera3D,
        srgb_output: bool,
    ) {
        let (width, height) = (size.0.max(1), size.1.max(1));
        let sun = self
            .sun
            .filter(|(dir, intensity)| dir.z > 0.0 && *intensity > 0.0);
        let sun_matrix = match (sun, self.bounds) {
            (Some((dir, _)), Some(bounds)) => sun_view_proj(dir, bounds),
            _ => DMat4::IDENTITY,
        };
        let (dir, intensity) = sun.unwrap_or((DVec3::Z, 0.0));
        #[allow(clippy::cast_possible_truncation)] // f32 достаточно для шейдера
        let globals = Globals3 {
            view_proj: mat(camera.view_proj(f64::from(width) / f64::from(height))),
            sun_view_proj: mat(sun_matrix),
            sun: [dir.x as f32, dir.y as f32, dir.z as f32, intensity as f32],
            params: [if srgb_output { 1.0 } else { 0.0 }, SHADOW_TEXEL, 0.0, 0.0],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            if sun.is_some()
                && let Some(mesh) = &self.scene_opaque
            {
                pass.set_pipeline(&self.shadow);
                pass.set_bind_group(0, &self.globals_group, &[]);
                mesh.draw(&mut pass);
            }
        }

        let depth = self.depth_view(device, width, height);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(sky(srgb_output)),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_bind_group(1, &self.shadow_group, &[]);
        if let Some(mesh) = &self.scene_opaque {
            pass.set_pipeline(&self.opaque);
            mesh.draw(&mut pass);
        }
        if let Some(mesh) = &self.scene_glass {
            pass.set_pipeline(&self.glass);
            mesh.draw(&mut pass);
        }
    }
}

/// Цвет неба для очистки кадра: в линейном пространстве для sRGB-поверхности.
fn sky(srgb_output: bool) -> wgpu::Color {
    if !srgb_output {
        return SKY;
    }
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    wgpu::Color {
        r: lin(SKY.r),
        g: lin(SKY.g),
        b: lin(SKY.b),
        a: 1.0,
    }
}
