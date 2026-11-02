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

fn selftest_append(line: &str) -> Result<(), String> {
    use std::io::Write as _;
    let path = selftest_path().ok_or("самопроверка не включена")?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|err| err.to_string())?;
    writeln!(file, "{line}").map_err(|err| err.to_string())
}

/// Включён ли режим самопроверки.
#[tauri::command]
fn selftest_enabled() -> bool {
    selftest_path().is_some()
}

/// Сценарий самопроверки (`OPENPLANNER_SELFTEST_SCENARIO`), например `dialogs`.
#[tauri::command]
fn selftest_scenario() -> Option<String> {
    std::env::var("OPENPLANNER_SELFTEST_SCENARIO")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Отмечает шаг сценария в отчёте: внешний драйвер ждёт этих строк, чтобы
/// пройти нативный диалог.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // команды Tauri получают аргументы по значению
fn selftest_step(step: String) -> Result<(), String> {
    selftest_append(&format!("STEP {step}"))
}

/// Записывает результат самопроверки и завершает приложение.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // команды Tauri получают аргументы по значению
fn selftest_report(app: tauri::AppHandle, ok: bool, details: String) -> Result<(), String> {
    let status = if ok { "OK" } else { "FAIL" };
    selftest_append(&format!("{status} {details}"))?;
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
            // Главное окно создаётся здесь (в конфиге `create: false`), чтобы
            // можно было передать настройки WebView до его создания.
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == "main")
                .cloned()
                .ok_or("в tauri.conf.json нет окна main")?;
            #[allow(unused_mut)] // изменяется только на Windows
            let mut builder = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?;
            #[cfg(windows)]
            if let Ok(args) = std::env::var("OPENPLANNER_WEBVIEW2_ARGS") {
                // Для проверок в CI (например, программный WebGPU-адаптер). Флаги
                // Tauri по умолчанию сохраняются.
                builder = builder.additional_browser_args(&format!(
                    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection {args}"
                ));
            }
            let window = builder.build()?;

            #[cfg(target_os = "linux")]
            // Флаг применяется к новым страницам: включаем и перезагружаем окно,
            // чтобы воркер движка создался уже с WebGL.
            window.with_webview(|webview| {
                let webview = webview.inner();
                if linux::enable_webgl_in_workers(&webview) {
                    webkit2gtk::WebViewExt::reload(&webview);
                }
            })?;
            #[cfg(not(target_os = "linux"))]
            let _ = window;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            selftest_enabled,
            selftest_scenario,
            selftest_step,
            selftest_report,
            files::read_project,
            files::read_image,
            files::read_scan,
            files::read_table,
            files::file_stamp,
            files::write_project,
            files::write_export,
            files::journal_path,
            files::journal_read,
            files::journal_reset,
            files::journal_append,
            files::journal_remove,
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить OpenPlanner");
}
