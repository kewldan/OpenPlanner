//! Нативная часть OpenPlanner: окно, файловая система, тяжёлые задачи.
//!
//! Модель и рендерер живут в WASM внутри вебвью; здесь — только сервисы.

/// Версия приложения для манифеста `.oplan` и окна «О программе».
#[tauri::command]
fn app_version() -> String {
    format!("OpenPlanner {}", env!("CARGO_PKG_VERSION"))
}

/// Запускает приложение.
///
/// # Panics
///
/// Падает, если Tauri не смог создать окно или цикл событий.
#[allow(clippy::expect_used)] // без окна приложению нечего делать
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("не удалось запустить OpenPlanner");
}
