use crate::{
    Camera2D,
    gpu::{GpuScene, Pipelines},
    gpu3d::Pipelines3D,
    scene::Scene,
    three::{Camera3D, Scene3D},
};

/// Слой сцены: документ меняется редко, оверлей инструмента — при каждом движении мыши.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneSlot {
    /// Документ.
    Document,
    /// Превью инструмента, выделение, привязки.
    Overlay,
}

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
    pipelines: Pipelines,
    document: GpuScene,
    overlay: GpuScene,
    three: Pipelines3D,
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

        let pipelines = Pipelines::new(&device, &queue, config.format);
        let three = Pipelines3D::new(&device, config.format);
        let document = GpuScene::new(&device);
        let overlay = GpuScene::new(&device);
        let backend = adapter.get_info().backend;
        log::info!("рендерер: бэкенд {backend:?}, формат {:?}", config.format);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            backend,
            pipelines,
            document,
            overlay,
            three,
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

    /// Загружает сцену в слой.
    pub fn set_scene(&mut self, slot: SceneSlot, scene: &Scene) {
        let target = match slot {
            SceneSlot::Document => &mut self.document,
            SceneSlot::Overlay => &mut self.overlay,
        };
        target.upload(&self.device, &self.queue, scene);
    }

    /// Загружает изображение RGBA8 под ключом (для [`ImageQuad`](crate::scene::ImageQuad)).
    pub fn set_image(&mut self, key: &str, width: u32, height: u32, rgba: &[u8]) {
        self.pipelines
            .set_image(&self.device, &self.queue, key, width, height, rgba);
    }

    /// Убирает изображение.
    pub fn remove_image(&mut self, key: &str) {
        self.pipelines.remove_image(key);
    }

    /// Загружено ли изображение.
    #[must_use]
    pub fn has_image(&self, key: &str) -> bool {
        self.pipelines.has_image(key)
    }

    /// Загружает 3D-сцену.
    pub fn set_scene_3d(&mut self, scene: &Scene3D) {
        self.three.upload(&self.device, &self.queue, scene);
    }

    /// Рисует кадр 3D-вида.
    pub fn render_3d(&mut self, camera: &Camera3D) {
        let Some(frame) = self.frame() else {
            return;
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame 3d"),
            });
        self.three.render(
            (&self.device, &self.queue),
            &mut encoder,
            &view,
            (self.config.width, self.config.height),
            camera,
            self.config.format.is_srgb(),
        );
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
    }

    /// Текстура кадра поверхности; `None` — кадр пропускается.
    fn frame(&mut self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.surface.configure(&self.device, &self.config);
                Some(frame)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                None
            }
            other => {
                log::warn!("кадр пропущен: {other:?}");
                None
            }
        }
    }

    /// Рисует кадр.
    ///
    /// `device_pixel_ratio` — отношение физических пикселей к CSS-пикселям.
    pub fn render(&mut self, camera: &Camera2D, device_pixel_ratio: f64) {
        let Some(frame) = self.frame() else {
            return;
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.pipelines.prepare(
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
            self.pipelines
                .draw(&mut pass, &[&self.document, &self.overlay]);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
    }
}
