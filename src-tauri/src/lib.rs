// GIF Editor — Tauri application library root
//
// All Tauri commands and plugin registrations live here.  The main.rs
// entry point is kept minimal; this lib is also the compilation unit
// used on mobile targets.

pub mod compositor;
pub mod error;
pub mod export;
pub mod fonts;
pub mod frame_source;
pub mod gif_decoder;
pub mod layer;
pub mod project;
pub mod text_renderer;
pub mod video_decoder;

mod commands;

use std::sync::Mutex;

use crate::project::{Project, ProjectState};

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(None::<Project>) as ProjectState)
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::open_gif,
            commands::get_frame,
            commands::add_image_layer,
            commands::add_text_layer,
            commands::update_layer,
            commands::remove_layer,
            commands::reorder_layers,
            commands::render_composite,
            commands::get_layers,
            commands::get_system_fonts,
            commands::export_project,
            commands::check_ffmpeg,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
