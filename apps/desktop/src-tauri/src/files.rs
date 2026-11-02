//! Файловые сервисы: проекты `.oplan` и журналы автосохранения.
//!
//! Команды принимают только пути с расширениями `.oplan` и `.oplan.journal`,
//! чтобы фронтенд не мог через них читать или писать произвольные файлы.

// Команды Tauri получают аргументы по значению — так устроен IPC.
#![allow(clippy::needless_pass_by_value)]

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use percent_encoding::percent_decode_str;
use tauri::{
    AppHandle, Manager,
    ipc::{InvokeBody, Request, Response},
};

/// Расширение файла проекта.
const PROJECT_EXT: &str = "oplan";
/// Заголовок с путём для бинарной записи (значение — percent-encoded UTF-8).
const PATH_HEADER: &str = "x-path";

type CmdResult<T> = Result<T, String>;

fn is_project(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == PROJECT_EXT)
}

fn is_journal(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext == op_format::journal::EXTENSION)
        && path.file_stem().is_some_and(|stem| {
            Path::new(stem)
                .extension()
                .is_some_and(|e| e == PROJECT_EXT)
        })
}

/// Изображения, которые можно добавить подложкой.
const IMAGE_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Файлы скана: сетки, облака точек, архивы экспорта, данные комнаты.
const SCAN_EXTENSIONS: [&str; 5] = ["glb", "zip", "ply", "obj", "json"];
/// Наибольший размер файла скана.
const MAX_SCAN_BYTES: u64 = 1 << 30;

fn is_scan(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SCAN_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

fn check(path: &Path, ok: fn(&Path) -> bool) -> CmdResult<()> {
    if ok(path) {
        Ok(())
    } else {
        Err(format!("недопустимый путь: {}", path.display()))
    }
}

fn io_error(action: &str, path: &Path, err: &std::io::Error) -> String {
    format!("не удалось {action} «{}»: {err}", path.display())
}

/// Записывает файл атомарно: во временный файл рядом, затем переименование.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Читает файл проекта целиком.
#[tauri::command]
pub fn read_project(path: PathBuf) -> CmdResult<Response> {
    check(&path, is_project)?;
    fs::read(&path)
        .map(Response::new)
        .map_err(|err| io_error("прочитать", &path, &err))
}

/// Читает изображение для подложки (PNG или JPEG).
#[tauri::command]
pub fn read_image(path: PathBuf) -> CmdResult<Response> {
    check(&path, is_image)?;
    fs::read(&path)
        .map(Response::new)
        .map_err(|err| io_error("прочитать", &path, &err))
}

/// Читает файл скана (GLB, ZIP, PLY, OBJ, JSON) до 1 ГБ.
#[tauri::command]
pub fn read_scan(path: PathBuf) -> CmdResult<Response> {
    check(&path, is_scan)?;
    let size = fs::metadata(&path)
        .map_err(|err| io_error("прочитать", &path, &err))?
        .len();
    if size > MAX_SCAN_BYTES {
        return Err(format!("файл «{}» больше 1 ГБ", path.display()));
    }
    fs::read(&path)
        .map(Response::new)
        .map_err(|err| io_error("прочитать", &path, &err))
}

/// Записывает файл проекта. Тело запроса — байты, путь — в заголовке `x-path`.
#[tauri::command]
pub fn write_project(request: Request<'_>) -> CmdResult<()> {
    let header = request
        .headers()
        .get(PATH_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or("не указан путь файла")?;
    let path = PathBuf::from(
        percent_decode_str(header)
            .decode_utf8()
            .map_err(|_| "путь файла не в UTF-8")?
            .into_owned(),
    );
    check(&path, is_project)?;
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("ожидались байты файла".into());
    };
    write_atomic(&path, bytes).map_err(|err| io_error("сохранить", &path, &err))
}

/// Путь журнала автосохранения: рядом с проектом или, для несохранённого
/// проекта, в папке данных приложения.
#[tauri::command]
pub fn journal_path(app: AppHandle, project: Option<PathBuf>) -> CmdResult<PathBuf> {
    if let Some(project) = project {
        check(&project, is_project)?;
        let mut path = project.into_os_string();
        path.push(".");
        path.push(op_format::journal::EXTENSION);
        return Ok(PathBuf::from(path));
    }
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("recovery");
    fs::create_dir_all(&dir).map_err(|err| io_error("создать папку", &dir, &err))?;
    Ok(dir.join("untitled.oplan.journal"))
}

/// Читает журнал. `None`, если журнала нет.
#[tauri::command]
pub fn journal_read(path: PathBuf) -> CmdResult<Option<String>> {
    check(&path, is_journal)?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(io_error("прочитать журнал", &path, &err)),
    }
}

/// Начинает журнал заново: перезаписывает его заголовком.
#[tauri::command]
pub fn journal_reset(path: PathBuf, header: String) -> CmdResult<()> {
    check(&path, is_journal)?;
    write_atomic(&path, header.as_bytes()).map_err(|err| io_error("создать журнал", &path, &err))
}

/// Дописывает строки в журнал.
#[tauri::command]
pub fn journal_append(path: PathBuf, text: String) -> CmdResult<()> {
    check(&path, is_journal)?;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(|err| io_error("открыть журнал", &path, &err))?;
    file.write_all(text.as_bytes())
        .map_err(|err| io_error("записать журнал", &path, &err))?;
    file.sync_data()
        .map_err(|err| io_error("записать журнал", &path, &err))
}

/// Удаляет журнал. Отсутствие журнала — не ошибка.
#[tauri::command]
pub fn journal_remove(path: PathBuf) -> CmdResult<()> {
    check(&path, is_journal)?;
    match fs::remove_file(&path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(io_error("удалить журнал", &path, &err))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_checks() {
        assert!(is_project(Path::new("/home/u/Ремонт/кв.oplan")));
        assert!(!is_project(Path::new("/etc/passwd")));
        assert!(is_image(Path::new("/home/u/план.JPG")));
        assert!(!is_image(Path::new("/home/u/план.svg")));
        assert!(is_scan(Path::new("/home/u/скан.GLB")));
        assert!(!is_scan(Path::new("/home/u/скан.exe")));
        assert!(is_journal(Path::new("/x/кв.oplan.journal")));
        assert!(!is_journal(Path::new("/x/кв.journal")));
        assert!(!is_journal(Path::new("/x/кв.oplan")));
    }

    #[test]
    fn journal_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("кв.oplan.journal");
        assert_eq!(journal_read(path.clone()).unwrap(), None);
        journal_reset(path.clone(), "h\n".into()).unwrap();
        journal_append(path.clone(), "a\n".into()).unwrap();
        journal_append(path.clone(), "b\n".into()).unwrap();
        assert_eq!(
            journal_read(path.clone()).unwrap().as_deref(),
            Some("h\na\nb\n")
        );
        journal_remove(path.clone()).unwrap();
        journal_remove(path.clone()).unwrap();
        assert_eq!(journal_read(path).unwrap(), None);
    }

    #[test]
    fn atomic_write_replaces_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("кв.oplan");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "временный файл не остался"
        );
    }

    #[test]
    fn rejects_foreign_paths() {
        assert!(read_project(PathBuf::from("/etc/passwd")).is_err());
        assert!(journal_append(PathBuf::from("/tmp/x.txt"), String::new()).is_err());
    }
}
