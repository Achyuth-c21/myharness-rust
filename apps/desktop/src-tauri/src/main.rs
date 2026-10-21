// Prevents an extra console window on Windows in release builds (tauri-v2 skill).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin passthrough: all application logic lives in lib.rs (required for mobile builds).
fn main() {
    myharness_desktop_lib::run()
}
