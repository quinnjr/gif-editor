use ab_glyph::FontArc;

use crate::error::AppError;

const BUNDLED_FONT: &[u8] = include_bytes!("../fonts/LiberationSans-Bold.ttf");

/// Load the font bundled with the binary. This is always available and
/// requires no filesystem access at runtime.
pub fn load_bundled_font() -> Result<FontArc, AppError> {
    FontArc::try_from_slice(BUNDLED_FONT)
        .map_err(|e| AppError::Font(format!("Failed to load bundled font: {e}")))
}

/// Resolve a font family name to a loaded FontArc.
///
/// MVP: the family name is ignored and the bundled font is always
/// returned.  A future implementation can walk system font directories
/// and match by name.
pub fn load_font(_family: &str) -> Result<FontArc, AppError> {
    load_bundled_font()
}

/// Return the list of font families the application currently supports.
pub fn list_available_fonts() -> Vec<String> {
    vec!["Impact".to_string()]
}
