use std::sync::OnceLock;

use crate::error::AppError;
use ab_glyph::FontArc;

// `static` (not `const`) so each embedded blob has one guaranteed address,
// letting `load_font` map the bytes chosen by `font_bytes_for_family` back
// to the matching parsed-font cache with a pointer comparison.
pub(crate) static LIBERATION_FONT: &[u8] = include_bytes!("../fonts/LiberationSans-Bold.ttf");
pub(crate) static ANTON_FONT: &[u8] = include_bytes!("../fonts/Anton-Regular.ttf");

// Parsed once per process; FontArc is an Arc, so clones are refcount bumps.
// Without this, the font tables are re-parsed on every text render (once per
// frame during export). `None` records a parse failure (unreachable for the
// embedded bytes in practice); the error is materialized per call because
// AppError is not Clone.
static LIBERATION: OnceLock<Option<FontArc>> = OnceLock::new();
static ANTON: OnceLock<Option<FontArc>> = OnceLock::new();

/// Resolve a font family name to the embedded TTF bytes it is served by.
///
/// Single source of the family→face mapping: "Impact" and "Anton"
/// (case-insensitive) map to Anton-Regular; any other family falls back to
/// LiberationSans-Bold.  Both `load_font` (export rasterisation) and the
/// `get_font_data` IPC command (font_data.rs, preview registration) route
/// through this so both render paths always resolve the same face.
pub(crate) fn font_bytes_for_family(family: &str) -> &'static [u8] {
    match family.to_lowercase().as_str() {
        "impact" | "anton" => ANTON_FONT,
        // Load-bearing fallback: pre-rename projects stored the legacy family
        // label "Liberation Sans Bold", which must keep resolving here.
        _ => LIBERATION_FONT,
    }
}

/// Parse `bytes` once per process into `cell` and hand out cheap clones.
fn parse_cached(
    cell: &'static OnceLock<Option<FontArc>>,
    bytes: &'static [u8],
    err: &str,
) -> Result<FontArc, AppError> {
    cell.get_or_init(|| FontArc::try_from_slice(bytes).ok())
        .clone()
        .ok_or_else(|| AppError::Font(err.to_string()))
}

pub fn load_bundled_font() -> Result<FontArc, AppError> {
    parse_cached(&LIBERATION, LIBERATION_FONT, "Failed to load bundled font")
}

/// Resolve a font family name to a loaded FontArc.
/// "Impact" and "Anton" both map to the bundled Anton-Regular.ttf.
/// Any other family name falls back to LiberationSans-Bold
/// (see [`font_bytes_for_family`] for the mapping).
pub fn load_font(family: &str) -> Result<FontArc, AppError> {
    let bytes = font_bytes_for_family(family);
    if std::ptr::eq(bytes, ANTON_FONT) {
        parse_cached(&ANTON, bytes, "Failed to load Anton font")
    } else {
        load_bundled_font()
    }
}

/// Return the list of font families the application currently supports.
/// The names match the CSS `@font-face` families the preview registers, so
/// a stored `font_family` resolves to the same face on both render paths.
pub fn list_available_fonts() -> Vec<String> {
    vec!["Anton".to_string(), "Liberation Sans".to_string()]
}
