//! Нативная часть OpenPlanner: окно, файловая система, тяжёлые задачи.
//!
//! Модель и рендерер живут в WASM внутри вебвью; здесь — только сервисы.

mod files;
#[cfg(target_os = "linux")]
mod linux;

/// Версия приложения для манифеста `.oplan` и окна «О программе».
#[tauri::command]
fn app_version() -> String {
    format!("OpenPlanner {}", env!("CARGO_PKG_VERSION"))
}

/// Файл отчёта самопроверки: если задан `OPENPLANNER_SELFTEST`, приложение
/// после запуска движка пишет туда результат и завершается (используется в CI).
fn selftest_path() -> Option<std::path::PathBuf> {
    std::env::var_os("OPENPLANNER_SELFTEST").map(Into::into)
}

/// Включён ли режим самопроверки.
#[tauri::command]
fn selftest_enabled() -> bool {
    selftest_path().is_some()
}

/// Записывает результат самопроверки и завершает приложение.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // команды Tauri получают аргументы по значению
fn selftest_report(app: tauri::AppHandle, ok: bool, details: String) -> Result<(), String> {
    let path = selftest_path().ok_or("самопроверка не включена")?;
    let status = if ok { "OK" } else { "FAIL" };
    std::fs::write(&path, format!("{status} {details}\n")).map_err(|err| err.to_string())?;
    app.exit(i32::from(!ok));
    Ok(())
}

/// Запускает приложение.
///
/// # Panics
///
/// Падает, если Tauri не смог создать окно или цикл событий.
#[allow(clippy::expect_used)] // без окна приложению нечего делать
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "linux")]
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                // Флаг применяется к новым страницам: включаем и перезагружаем окно,
                // чтобы воркер движка создался уже с WebGL.
                window.with_webview(|webview| {
                    let webview = webview.inner();
                    if linux::enable_webgl_in_workers(&webview) {
                        webkit2gtk::WebViewExt::reload(&webview);
                    }
                })?;
            }
            #[cfg(not(target_os = "linux"))]
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            selftest_enabled,
            selftest_report,
            files::read_project,
            files::write_project,
            files::journal_path,
            files::journal_read,
            files::journal_reset,
            files::journal_append,
            files::journal_remove,
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить OpenPlanner");
}
