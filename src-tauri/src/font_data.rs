// Serves the embedded font bytes to the WebKit preview over IPC.
//
// The TTFs are compiled into the binary once (src/fonts.rs); the frontend
// fetches them via this command and registers them with the JS FontFace
// API instead of shipping a second copy under static/fonts.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::error::AppError;
use crate::fonts::font_bytes_for_family;

/// Return the embedded TTF bytes for a font family, base64-encoded.
///
/// Family resolution shares [`font_bytes_for_family`] with
/// `fonts::load_font`, so preview and export rasterise the same glyphs.
#[tauri::command]
pub fn get_font_data(family: String) -> Result<String, AppError> {
    Ok(STANDARD.encode(font_bytes_for_family(&family)))
}
