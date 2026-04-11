use ab_glyph::FontArc;
use crate::error::AppError;

const LIBERATION_FONT: &[u8] = include_bytes!("../fonts/LiberationSans-Bold.ttf");
const ANTON_FONT: &[u8] = include_bytes!("../fonts/Anton-Regular.ttf");

pub fn load_bundled_font() -> Result<FontArc, AppError> {
    FontArc::try_from_slice(LIBERATION_FONT)
        .map_err(|e| AppError::Font(format!("Failed to load bundled font: {e}")))
}

/// Resolve a font family name to a loaded FontArc.
/// "Impact" and "Anton" both map to the bundled Anton-Regular.ttf.
/// Any other family name falls back to LiberationSans-Bold.
pub fn load_font(family: &str) -> Result<FontArc, AppError> {
    match family.to_lowercase().as_str() {
        "impact" | "anton" => FontArc::try_from_slice(ANTON_FONT)
            .map_err(|e| AppError::Font(format!("Failed to load Anton font: {e}"))),
        _ => load_bundled_font(),
    }
}

/// Return the list of font families the application currently supports.
pub fn list_available_fonts() -> Vec<String> {
    vec!["Anton".to_string(), "Liberation Sans Bold".to_string()]
}
