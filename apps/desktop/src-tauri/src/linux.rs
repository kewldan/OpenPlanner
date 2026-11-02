//! Настройка WebKitGTK (Linux).
//!
//! Движок рисует через WebGL2 в `OffscreenCanvas` внутри Web Worker. В
//! WebKitGTK эта возможность (`AllowWebGLInWorkers`) стабильна, но выключена
//! по умолчанию, и без неё холст не запускается. Включаем её через API флагов
//! WebKitGTK (2.42+), которого нет в Rust-обёртке `webkit2gtk`, поэтому
//! функции объявлены вручную.

// Вызовы C API WebKitGTK; инварианты описаны у каждого блока.
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int, c_void};

use webkit2gtk::{WebView, WebViewExt, glib::translate::ToGlibPtr};

/// Флаг `WebKit`: `WebGL` в воркерах (для `OffscreenCanvas.getContext("webgl2")`).
const WEBGL_IN_WORKERS: &CStr = c"AllowWebGLInWorkers";

#[link(name = "webkit2gtk-4.1")]
unsafe extern "C" {
    fn webkit_settings_get_all_features() -> *mut c_void;
    fn webkit_feature_list_get_length(list: *mut c_void) -> usize;
    fn webkit_feature_list_get(list: *mut c_void, index: usize) -> *mut c_void;
    fn webkit_feature_list_unref(list: *mut c_void);
    fn webkit_feature_get_identifier(feature: *mut c_void) -> *const c_char;
    fn webkit_settings_set_feature_enabled(settings: *mut c_void, feature: *mut c_void, on: c_int);
}

/// Включает WebGL в воркерах. Возвращает `false`, если такого флага нет.
pub fn enable_webgl_in_workers(webview: &WebView) -> bool {
    let Some(settings) = WebViewExt::settings(webview) else {
        return false;
    };
    let settings_ptr: *mut webkit2gtk::ffi::WebKitSettings = settings.to_glib_none().0;
    // SAFETY: список возвращается с собственной ссылкой и освобождается ниже;
    // элементы списка живут, пока жив список; идентификатор — статическая
    // C-строка WebKit; `settings_ptr` действителен, пока жив `settings`.
    unsafe {
        let list = webkit_settings_get_all_features();
        if list.is_null() {
            return false;
        }
        let mut found = false;
        for index in 0..webkit_feature_list_get_length(list) {
            let feature = webkit_feature_list_get(list, index);
            let id = webkit_feature_get_identifier(feature);
            if !id.is_null() && CStr::from_ptr(id) == WEBGL_IN_WORKERS {
                webkit_settings_set_feature_enabled(settings_ptr.cast(), feature, 1);
                found = true;
                break;
            }
        }
        webkit_feature_list_unref(list);
        found
    }
}
