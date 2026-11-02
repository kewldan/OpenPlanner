// Без консольного окна в релизной сборке под Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    openplanner_desktop_lib::run();
}
