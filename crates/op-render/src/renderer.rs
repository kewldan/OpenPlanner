use crate::{Camera2D, grid::GridPipeline};

/// Ошибки рендерера.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Не найден подходящий GPU-адаптер.
    #[error("не найден графический адаптер: {0}")]
    Adapter(String),
    /// Не удалось создать устройство.
    #[error("не удалось инициализировать GPU: {0}")]
    Device(String),
    /// Поверхность не поддерживается адаптером.
    #[error("холст не поддерживается графическим адаптером")]
    UnsupportedSurface,
}

/// Рендерер холста.
#[derive(Debug)]
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    backend: wgpu::Backend,
    grid: GridPipeline,
}

impl Renderer {
    /// Создаёт рендерер для готовой поверхности.
    ///
    /// # Errors
    ///
    /// Возвращает [`RenderError`], если нет подходящего адаптера или устройства.
    pub async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|err| RenderError::Adapter(err.to_string()))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("openplanner"),
                required_features: wgpu::Features::empty(),
                // WebGL2 — самый ограниченный бэкенд; лимиты подстраиваем под адаптер.
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|err| RenderError::Device(err.to_string()))?;

        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or(RenderError::UnsupportedSurface)?;
        // Предпочитаем не-sRGB формат: цвета дизайна заданы в sRGB и выводятся как есть.
        let caps = surface.get_capabilities(&adapter);
        if let Some(format) = caps.formats.iter().copied().find(|f| !f.is_srgb()) {
            config.format = format;
        }
        surface.configure(&device, &config);

        let grid = GridPipeline::new(&device, config.format);
        let backend = adapter.get_info().backend;
        log::info!("рендерер: бэкенд {backend:?}, формат {:?}", config.format);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            backend,
            grid,
        })
    }

    /// Название графического бэкенда, например `WebGpu` или `Gl`.
    #[must_use]
    pub fn backend_name(&self) -> String {
        format!("{:?}", self.backend)
    }

    /// Меняет размер холста в физических пикселях.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Рисует кадр.
    ///
    /// `device_pixel_ratio` — отношение физических пикселей к CSS-пикселям.
    pub fn render(&mut self, camera: &Camera2D, device_pixel_ratio: f64) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            other => {
                log::warn!("кадр пропущен: {other:?}");
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.grid.prepare(
            &self.queue,
            camera,
            (self.config.width, self.config.height),
            device_pixel_ratio,
            self.config.format.is_srgb(),
        );

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plan"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.grid.draw(&mut pass);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
    }
}
