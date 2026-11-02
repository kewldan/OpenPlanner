//! WASM-обёртка: связывает [`EngineState`] с рендерером и `OffscreenCanvas`.

use glam::DVec2;
use op_render::{Renderer, SceneSlot};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{
    EngineState,
    protocol::{EngineEvents, ScanImport, ToEngine},
};

/// Распознаёт скан (GLB, OBJ, PLY, ZIP, RoomPlan JSON). Вызывается в
/// отдельном воркере: `progress(stage, count)` сообщает номер этапа.
///
/// # Errors
///
/// Файл не читается или в нём нет геометрии.
#[wasm_bindgen(js_name = analyzeScan)]
pub fn analyze_scan(
    bytes: &[u8],
    progress: &js_sys::Function,
) -> Result<Ts<op_scan::ScanResult>, JsError> {
    let mut report = |stage: op_scan::Stage| {
        let _ = progress.call2(
            &JsValue::NULL,
            &JsValue::from(stage.index()),
            &JsValue::from(op_scan::Stage::COUNT),
        );
    };
    let result =
        op_scan::analyze(bytes, &mut report).map_err(|err| JsError::new(&err.to_string()))?;
    Ok(result.into_ts()?)
}

/// Движок, живущий в Web Worker.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Engine {
    state: EngineState,
    renderer: Renderer,
}

#[wasm_bindgen]
impl Engine {
    /// Создаёт движок для холста. `width` и `height` — CSS-пиксели.
    ///
    /// # Errors
    ///
    /// Ошибка, если в окружении нет ни WebGPU, ни WebGL2.
    pub async fn create(
        canvas: web_sys::OffscreenCanvas,
        width: f64,
        height: f64,
        device_pixel_ratio: f64,
    ) -> Result<Engine, JsError> {
        console_error_panic_hook::set_once();

        let state = EngineState::new(DVec2::new(width, height), device_pixel_ratio);
        let (px_w, px_h) = physical_size(&state);
        canvas.set_width(px_w);
        canvas.set_height(px_h);

        // WebGPU, если он есть, иначе WebGL2.
        let instance = wgpu::util::new_instance_with_webgpu_detection(
            wgpu::InstanceDescriptor::new_without_display_handle(),
        )
        .await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas))
            .map_err(|err| JsError::new(&format!("не удалось создать холст: {err}")))?;
        let renderer = Renderer::new(&instance, surface, px_w, px_h)
            .await
            .map_err(|err| JsError::new(&err.to_string()))?;

        let mut engine = Self { state, renderer };
        engine.sync();
        Ok(engine)
    }

    /// Передаёт рендереру изменившиеся сцены и новые изображения.
    fn sync(&mut self) {
        for (key, image) in self.state.take_image_uploads() {
            self.renderer
                .set_image(&key, image.width, image.height, &image.rgba);
        }
        let (document, overlay) = self.state.take_scenes();
        if let Some(scene) = document {
            self.renderer.set_scene(SceneSlot::Document, &scene);
        }
        if let Some(scene) = overlay {
            self.renderer.set_scene(SceneSlot::Overlay, &scene);
        }
    }

    /// Графический бэкенд: `BrowserWebGpu` или `Gl`.
    #[wasm_bindgen(getter)]
    pub fn backend(&self) -> String {
        self.renderer.backend_name()
    }

    /// События, описывающие текущее состояние целиком.
    ///
    /// # Errors
    ///
    /// Ошибка сериализации (не должна возникать).
    pub fn snapshot(&self) -> Result<Ts<EngineEvents>, JsError> {
        Ok(self.state.snapshot().into_ts()?)
    }

    /// Обрабатывает сообщение UI и возвращает события.
    ///
    /// # Errors
    ///
    /// Ошибка, если сообщение не соответствует протоколу.
    #[allow(clippy::needless_pass_by_value)] // wasm-bindgen принимает аргументы только по значению
    pub fn handle(&mut self, message: Ts<ToEngine>) -> Result<Ts<EngineEvents>, JsError> {
        let message = message.to_rust()?;
        let resized = matches!(message, ToEngine::Resize { .. });
        let events = self.state.handle(message);
        if resized {
            let (w, h) = physical_size(&self.state);
            self.renderer.resize(w, h);
        }
        Ok(events.into_ts()?)
    }

    /// Записывает проект в байты файла `.oplan`.
    ///
    /// # Errors
    ///
    /// Ошибка сериализации.
    #[wasm_bindgen(js_name = exportFile)]
    pub fn export_file(&self, generator: &str) -> Result<Vec<u8>, JsError> {
        Ok(self.state.export_file(generator)?)
    }

    /// Открывает проект из байтов файла `.oplan`.
    ///
    /// # Errors
    ///
    /// Ошибка, если файл повреждён или создан более новой версией.
    #[wasm_bindgen(js_name = importFile)]
    pub fn import_file(&mut self, bytes: &[u8]) -> Result<Ts<EngineEvents>, JsError> {
        Ok(self.state.import_file(bytes)?.into_ts()?)
    }

    /// Импортирует распознанный скан. `source` — исходный файл (сохраняется в проекте).
    ///
    /// # Errors
    ///
    /// Запрос не соответствует протоколу.
    #[wasm_bindgen(js_name = importScan)]
    #[allow(clippy::needless_pass_by_value)] // wasm-bindgen принимает аргументы только по значению
    pub fn import_scan(
        &mut self,
        request: Ts<ScanImport>,
        source: Option<Vec<u8>>,
    ) -> Result<Ts<EngineEvents>, JsError> {
        let request = request.to_rust()?;
        Ok(self.state.import_scan(&request, source).into_ts()?)
    }

    /// Добавляет изображение-подложку (PNG или JPEG).
    ///
    /// # Errors
    ///
    /// Ошибка, если файл не читается как изображение.
    #[wasm_bindgen(js_name = addUnderlay)]
    pub fn add_underlay(
        &mut self,
        bytes: Vec<u8>,
        name: &str,
    ) -> Result<Ts<EngineEvents>, JsError> {
        let events = self
            .state
            .add_underlay(bytes, name)
            .map_err(|err| JsError::new(&err))?;
        Ok(events.into_ts()?)
    }

    /// Воспроизводит журнал автосохранения поверх текущего документа.
    ///
    /// # Errors
    ///
    /// Ошибка разбора журнала.
    #[wasm_bindgen(js_name = replayJournal)]
    pub fn replay_journal(&mut self, text: &str) -> Result<Ts<EngineEvents>, JsError> {
        Ok(self.state.replay_journal(text)?.into_ts()?)
    }

    /// Первая строка нового журнала автосохранения.
    ///
    /// # Errors
    ///
    /// Ошибка сериализации (не должна возникать).
    #[wasm_bindgen(js_name = journalHeader)]
    pub fn journal_header() -> Result<String, JsError> {
        Ok(op_format::journal::header_line()?)
    }

    /// Рисует кадр.
    pub fn render(&mut self) {
        self.sync();
        self.renderer
            .render(self.state.camera(), self.state.device_pixel_ratio());
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // размеры холста > 0 и < 2^32
fn physical_size(state: &EngineState) -> (u32, u32) {
    let size = (state.viewport() * state.device_pixel_ratio())
        .round()
        .max(DVec2::ONE);
    (size.x as u32, size.y as u32)
}
