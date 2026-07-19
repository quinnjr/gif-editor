// Tests for the bundled font loading and enumeration in fonts.rs, and for
// the get_font_data IPC command in font_data.rs.

use ab_glyph::Font;
use gif_editor_lib::font_data::get_font_data;
use gif_editor_lib::fonts::{list_available_fonts, load_bundled_font, load_font};

#[test]
fn impact_and_anton_map_to_bundled_anton() {
    let impact = load_font("Impact").unwrap();
    let anton = load_font("Anton").unwrap();
    // Both family names resolve to the same bundled Anton-Regular face.
    assert_eq!(impact.glyph_count(), anton.glyph_count());
    // Matching is case-insensitive.
    let lower = load_font("impact").unwrap();
    assert_eq!(lower.glyph_count(), anton.glyph_count());
    // Anton is a different face from the Liberation fallback.
    let liberation = load_bundled_font().unwrap();
    assert_ne!(anton.glyph_count(), liberation.glyph_count());
}

#[test]
fn unknown_family_falls_back_to_liberation() {
    let fallback = load_font("Definitely Not A Real Font").unwrap();
    let liberation = load_bundled_font().unwrap();
    assert_eq!(fallback.glyph_count(), liberation.glyph_count());
}

#[test]
fn list_available_fonts_contains_bundled_families() {
    let fonts = list_available_fonts();
    assert!(!fonts.is_empty());
    assert!(fonts.iter().any(|f| f == "Anton"));
    assert!(fonts.iter().any(|f| f == "Liberation Sans"));
}

// ---------------------------------------------------------------------------
// get_font_data (font_data.rs)
// ---------------------------------------------------------------------------

// The same TTFs the library embeds, included independently so the tests can
// verify the served bytes without access to the pub(crate) constants.
const ANTON_BYTES: &[u8] = include_bytes!("../fonts/Anton-Regular.ttf");
const LIBERATION_BYTES: &[u8] = include_bytes!("../fonts/LiberationSans-Bold.ttf");

/// Standard-alphabet base64 with `=` padding — the same encoding as the
/// `base64::engine::general_purpose::STANDARD` engine get_font_data uses.
/// Hand-rolled here because `base64` is a lib dependency only, not a
/// dev-dependency, and integration tests only link the lib itself.
fn base64_standard(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[test]
fn get_font_data_impact_and_anton_serve_anton_bytes() {
    let expected = base64_standard(ANTON_BYTES);
    assert_eq!(
        get_font_data("Impact".to_string()).unwrap(),
        expected,
        "\"Impact\" must serve the Anton bytes"
    );
    // Matching is case-insensitive, as in load_font.
    assert_eq!(
        get_font_data("anton".to_string()).unwrap(),
        expected,
        "\"anton\" must serve the Anton bytes"
    );
}

#[test]
fn get_font_data_unknown_family_serves_liberation_bytes() {
    assert_eq!(
        get_font_data("Arial".to_string()).unwrap(),
        base64_standard(LIBERATION_BYTES),
        "unmapped families must fall back to the Liberation bytes"
    );
}

/// Guards the two assertions above against degenerating into a tautology:
/// the Anton and Liberation payloads must actually differ.
#[test]
fn get_font_data_anton_and_liberation_bytes_differ() {
    assert_ne!(ANTON_BYTES, LIBERATION_BYTES);
    assert_ne!(
        get_font_data("Anton".to_string()).unwrap(),
        get_font_data("Arial".to_string()).unwrap()
    );
}
