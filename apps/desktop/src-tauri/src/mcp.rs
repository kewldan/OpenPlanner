//! MCP: мост между агентом (stdio) и открытым окном OpenPlanner.
//!
//! Приложение слушает локальный TCP-порт (только 127.0.0.1) и пускает
//! соединения с токеном из файла `mcp.json` в папке данных приложения (файл
//! доступен только пользователю). Строки JSON-RPC уходят в окно событием
//! `mcp-request`; ответ возвращается командой `mcp_reply`. Сам протокол и
//! инструменты — в движке (`op_engine::mcp`).
//!
//! `openplanner-desktop mcp` — мост для MCP-клиента: читает stdin и пишет
//! stdout, а если приложение не запущено — запускает его и ждёт.

use std::{
    collections::HashMap,
    hash::{BuildHasher as _, Hasher as _},
    io::{BufRead as _, BufReader, Write as _},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter as _, Manager as _};

/// Идентификатор приложения (как в `tauri.conf.json`) — имя папки данных.
const IDENTIFIER: &str = "app.openplanner.desktop";
/// Сколько мост ждёт запуска приложения.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Адрес и токен работающего сервера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    /// Порт на 127.0.0.1.
    pub port: u16,
    /// Токен — первая строка соединения.
    pub token: String,
}

/// Строка от агента для окна.
#[derive(Debug, Clone, Serialize)]
struct Request {
    conn: u64,
    line: String,
}

/// Соединения агентов и очередь до готовности окна.
#[derive(Default)]
pub struct Server {
    writers: Mutex<HashMap<u64, TcpStream>>,
    ready: AtomicBool,
    pending: Mutex<Vec<Request>>,
}

/// Папка данных приложения без `AppHandle` (для моста): как у Tauri `app_data_dir`.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|b| b.join(IDENTIFIER))
}

/// Случайный токен из случайных ключей хешера стандартной библиотеки.
fn token() -> String {
    (0..4)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Записывает `mcp.json`, доступный только пользователю.
fn write_endpoint(path: &std::path::Path, endpoint: &Endpoint) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec(endpoint).map_err(std::io::Error::other)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)?.write_all(&json)
}

/// Читает адрес сервера из `mcp.json`.
#[must_use]
pub fn read_endpoint(path: &std::path::Path) -> Option<Endpoint> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

impl Server {
    fn deliver(&self, app: &AppHandle, request: Request) {
        if self.ready.load(Ordering::SeqCst) {
            let _ = app.emit("mcp-request", request);
        } else if let Ok(mut pending) = self.pending.lock() {
            pending.push(request);
        }
    }

    fn connection(self: &Arc<Self>, app: &AppHandle, stream: TcpStream, conn: u64, token: &str) {
        let Ok(reader) = stream.try_clone() else {
            return;
        };
        let mut lines = BufReader::new(reader).lines();
        // Чужие соединения без токена не пускаем.
        match lines.next() {
            Some(Ok(line)) if line.trim() == token => {}
            _ => return,
        }
        if let Ok(mut writers) = self.writers.lock() {
            writers.insert(conn, stream);
        }
        for line in lines.map_while(Result::ok) {
            if !line.trim().is_empty() {
                self.deliver(app, Request { conn, line });
            }
        }
        if let Ok(mut writers) = self.writers.lock() {
            writers.remove(&conn);
        }
    }

    /// Ответ агенту.
    fn reply(&self, conn: u64, line: &str) -> Result<(), String> {
        let mut writers = self.writers.lock().map_err(|e| e.to_string())?;
        let stream = writers.get_mut(&conn).ok_or("агент отключился")?;
        writeln!(stream, "{line}").map_err(|e| e.to_string())
    }
}

/// Запускает сервер MCP приложения: порт, токен в `mcp.json`, приём соединений.
///
/// # Errors
///
/// Порт не открылся или файл не записался.
pub fn serve(app: &AppHandle) -> Result<Arc<Server>, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let endpoint = Endpoint {
        port,
        token: token(),
    };
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("mcp.json");
    write_endpoint(&path, &endpoint).map_err(|e| e.to_string())?;
    let server = Arc::new(Server::default());
    let next = Arc::new(AtomicU64::new(1));
    let (accept, app) = (Arc::clone(&server), app.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming().map_while(Result::ok) {
            let conn = next.fetch_add(1, Ordering::SeqCst);
            let (server, app, token) = (Arc::clone(&accept), app.clone(), endpoint.token.clone());
            std::thread::spawn(move || server.connection(&app, stream, conn, &token));
        }
    });
    Ok(server)
}

/// Окно готово принимать запросы агента: накопленные уходят сразу.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // команды Tauri получают аргументы по значению
pub fn mcp_ready(app: AppHandle, server: tauri::State<'_, Arc<Server>>) {
    server.ready.store(true, Ordering::SeqCst);
    let pending = server
        .pending
        .lock()
        .map(|mut p| std::mem::take(&mut *p))
        .unwrap_or_default();
    for request in pending {
        let _ = app.emit("mcp-request", request);
    }
}

/// Ответ окна агенту.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // команды Tauri получают аргументы по значению
pub fn mcp_reply(
    server: tauri::State<'_, Arc<Server>>,
    conn: u64,
    line: String,
) -> Result<(), String> {
    server.reply(conn, &line)
}

/// Соединение с приложением: из `mcp.json`, при необходимости — запуск приложения.
fn connect() -> Result<TcpStream, String> {
    let path = data_dir()
        .ok_or("не найдена папка данных")?
        .join("mcp.json");
    let attempt = || {
        let endpoint = read_endpoint(&path)?;
        let mut stream = TcpStream::connect(("127.0.0.1", endpoint.port)).ok()?;
        writeln!(stream, "{}", endpoint.token).ok()?;
        Some(stream)
    };
    if let Some(stream) = attempt() {
        return Ok(stream);
    }
    // Приложение не запущено: открыть его и дождаться сервера.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("не удалось запустить OpenPlanner: {e}"))?;
    let start = Instant::now();
    while start.elapsed() < LAUNCH_TIMEOUT {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(stream) = attempt() {
            return Ok(stream);
        }
    }
    Err("OpenPlanner не ответил: откройте приложение и повторите".into())
}

/// Мост stdio ↔ приложение. Возвращает код выхода.
#[must_use]
pub fn bridge() -> i32 {
    let stream = match connect() {
        Ok(s) => s,
        Err(e) => {
            // MCP-клиенты показывают stderr сервера в своём журнале.
            #[allow(clippy::print_stderr)]
            {
                eprintln!("openplanner mcp: {e}");
            }
            return 1;
        }
    };
    let Ok(reader) = stream.try_clone() else {
        return 1;
    };
    // Приложение → агент.
    std::thread::spawn(move || {
        let mut out = std::io::stdout().lock();
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
                break;
            }
        }
        // Приложение закрыли — агенту больше некуда писать.
        std::process::exit(0);
    });
    // Агент → приложение.
    let mut writer = stream;
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        if writeln!(writer, "{line}").is_err() {
            return 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_round_trip_and_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/mcp.json");
        let e = Endpoint {
            port: 4242,
            token: token(),
        };
        write_endpoint(&path, &e).unwrap();
        assert_eq!(read_endpoint(&path), Some(e.clone()));
        assert_eq!(e.token.len(), 64);
        assert_ne!(e.token, token());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn data_dir_ends_with_the_identifier() {
        if let Some(d) = data_dir() {
            assert!(d.ends_with(IDENTIFIER));
        }
    }
}
