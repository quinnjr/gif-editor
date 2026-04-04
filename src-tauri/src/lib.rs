// GIF Editor — Tauri application library root
//
// All Tauri commands and plugin registrations live here. The main.rs
// entry point is kept minimal; this lib is also the compilation unit
// used on mobile targets.

pub mod error;
pub mod gif_decoder;
pub mod layer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
