//! Фотореалистичный рендер: сцена текущего вида для трассировки в отдельном
//! воркере (`op_render::trace`) и готовые кадры в проекте (`assets/renders/`).

use op_model::{Command, RenderShot};
use op_render::trace;

use crate::{
    EngineState,
    camera3d::Camera3d,
    protocol::{EngineEvents, FromEngine, Options3d, SunTime},
};

/// Солнце по умолчанию для рендера без настроенного солнца: 21 июня, 13:00.
const DEFAULT_SUN: SunTime = SunTime {
    day: 172,
    hour: 13.0,
};

impl EngineState {
    /// Сцена и камера для трассировки одним блоком байт: ракурс `view` или
    /// текущая камера 3D-вида. Срез выключен, солнце — из настроек 3D или летний
    /// полдень; потолки — как в 3D-виде, но с камеры выше потолка их нет
    /// («кукольный домик»: иначе снаружи видна только крыша).
    ///
    /// # Errors
    ///
    /// Ракурса нет или сцена пуста.
    pub fn render_input(&self, view: Option<&str>) -> Result<Vec<u8>, String> {
        let mut options = Options3d {
            enabled: true,
            white: false,
            cut: None,
            sun: Some(self.options_3d.sun.unwrap_or(DEFAULT_SUN)),
            ..self.options_3d
        };
        let probe = self.scene_3d_with(&options);
        if probe.opaque.indices.is_empty() {
            return Err("Нечего рендерить: нарисуйте стены".into());
        }
        let camera = match view {
            Some(id) => {
                let saved = self
                    .history
                    .document()
                    .views
                    .iter()
                    .find(|v| v.id == id)
                    .ok_or("Такого ракурса нет")?;
                let mut c = Camera3d::default();
                c.restore(saved);
                c.render_camera()
            }
            None if self.camera_3d_fitted => self.camera_3d.render_camera(),
            None => {
                let mut c = Camera3d::default();
                if let Some(bounds) = probe.opaque.bounds() {
                    c.fit(bounds);
                }
                c.render_camera()
            }
        };
        let ceiling = self.history.document().default_ceiling_height;
        if camera.eye.z > ceiling {
            options.ceiling = false;
        }
        let scene = self.scene_3d_with(&options);
        Ok(trace::encode(&scene, &camera))
    }

    /// Сохраняет готовый кадр (PNG) в проекте одним действием.
    ///
    /// # Errors
    ///
    /// Байты — не PNG.
    pub fn add_render(
        &mut self,
        bytes: Vec<u8>,
        name: &str,
        passes: u32,
        date: &str,
    ) -> Result<EngineEvents, String> {
        let image = crate::decode_image(&bytes)?;
        let id = self.ids.generate();
        let asset = format!("assets/renders/{id}.png");
        let mut renders = self.history.document().renders.clone();
        renders.push(RenderShot {
            id,
            name: name.to_owned(),
            asset: asset.clone(),
            width: image.width,
            height: image.height,
            passes,
            date: date.to_owned(),
        });
        Ok(EngineEvents {
            events: self.execute_all(vec![
                Command::asset(asset, bytes),
                Command::SetRenders { renders },
            ]),
        })
    }

    /// Удаляет кадр вместе с файлом.
    pub(crate) fn delete_render(&mut self, id: &str) -> Vec<FromEngine> {
        let doc = self.history.document();
        let Some(shot) = doc.renders.iter().find(|r| r.id == id).cloned() else {
            return vec![FromEngine::Error {
                message: "Такого кадра нет".into(),
            }];
        };
        let renders = doc.renders.iter().filter(|r| r.id != id).cloned().collect();
        self.execute_all(vec![
            Command::SetRenders { renders },
            Command::RemoveAsset { path: shot.asset },
        ])
    }

    /// Байты файла проекта (картинки кадров для галереи).
    #[must_use]
    pub fn asset_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.history
            .document()
            .assets
            .get(path)
            .map(|d| d.as_ref().clone())
    }
}

/// Трассирует сцену из [`EngineState::render_input`] и возвращает PNG.
/// `progress(pass)` вызывается после каждого прохода.
///
/// # Errors
///
/// Повреждённые входные данные.
pub fn trace_png(
    input: &[u8],
    width: u32,
    height: u32,
    passes: u32,
    progress: impl FnMut(u32) -> bool,
) -> Result<Vec<u8>, String> {
    let (scene, camera) = trace::decode(input).ok_or("сцена рендера повреждена")?;
    let (width, height) = (width.clamp(16, 4096), height.clamp(16, 4096));
    let frame = trace::render(&scene, &camera, width, height, passes.max(1), progress);
    let image = image::RgbaImage::from_raw(width, height, frame.rgba()).ok_or("кадр")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}
