//! Rasterise a [`TextLayer`] into an RGBA image buffer.
//!
//! The renderer uses `ab_glyph` for glyph layout and coverage maps.
//! Stroke outlines are produced by drawing the text at a ring of
//! offset positions (with the stroke colour) before drawing the fill
//! text on top.  The output is a tight bounding-box image sized to
//! exactly the rendered text; positioning onto a frame canvas is the
//! compositor's responsibility.

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

use crate::error::AppError;
use crate::fonts::load_font;
use crate::layer::TextLayer;

/// Render `layer` to an RGBA image.
///
/// Returns a 1×1 transparent image for empty text so that callers
/// never have to handle a zero-dimension surface.
pub fn render_text(layer: &TextLayer) -> Result<RgbaImage, AppError> {
    if layer.text.is_empty() {
        return Ok(RgbaImage::new(1, 1));
    }

    let font = load_font(&layer.font_family)?;
    let scale = PxScale::from(layer.font_size as f32);

    // --- measure the text extent ---
    let (text_w, text_h, ascent) = measure_text(&font, &layer.text, scale);
    if text_w == 0 || text_h == 0 {
        return Ok(RgbaImage::new(1, 1));
    }

    // Extra padding around the glyph bounding box so strokes aren't clipped.
    let stroke_pad = layer
        .stroke
        .as_ref()
        .map(|s| s.width.ceil() as u32 + 2)
        .unwrap_or(0);
    let pad = stroke_pad;

    let img_w = text_w + pad * 2;
    let img_h = text_h + pad * 2;
    let mut img = RgbaImage::new(img_w, img_h);

    let x0 = pad as f32;
    let y0 = pad as f32 + ascent;

    // --- stroke pass (drawn first so fill sits on top) ---
    if let Some(ref stroke) = layer.stroke {
        let offsets = generate_stroke_offsets(stroke.width as f32);
        let stroke_color = Rgba(stroke.color);
        for (dx, dy) in &offsets {
            draw_text_at(
                &mut img,
                &font,
                &layer.text,
                scale,
                x0 + dx,
                y0 + dy,
                stroke_color,
            );
        }
    }

    // --- fill pass ---
    let fill_color = Rgba(layer.color);
    draw_text_at(&mut img, &font, &layer.text, scale, x0, y0, fill_color);

    Ok(img)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Measure the pixel dimensions of `text` at `scale`.
///
/// Returns `(width, height, ascent)` where ascent is the distance from
/// the baseline to the top of the em square (used to position glyphs
/// so the top of the image aligns with the top of the tallest glyph).
fn measure_text(font: &FontArc, text: &str, scale: PxScale) -> (u32, u32, f32) {
    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();
    let descent = scaled.descent(); // negative value

    let height = (ascent - descent).ceil() as u32;

    let mut width: f32 = 0.0;
    let mut prev_glyph_id = None;
    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        if let Some(prev) = prev_glyph_id {
            width += scaled.kern(prev, glyph_id);
        }
        width += scaled.h_advance(glyph_id);
        prev_glyph_id = Some(glyph_id);
    }

    (width.ceil() as u32, height, ascent)
}

/// Rasterise `text` onto `img` with the top-left of the first glyph at
/// pixel (`x`, baseline_y`).  `baseline_y` is measured downward from
/// the top of the image to the text baseline.
fn draw_text_at(
    img: &mut RgbaImage,
    font: &FontArc,
    text: &str,
    scale: PxScale,
    x: f32,
    baseline_y: f32,
    color: Rgba<u8>,
) {
    let scaled = font.as_scaled(scale);
    let mut cursor_x = x;
    let mut prev_glyph_id = None;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        if let Some(prev) = prev_glyph_id {
            cursor_x += scaled.kern(prev, glyph_id);
        }

        let glyph: Glyph =
            glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));
        cursor_x += scaled.h_advance(glyph_id);
        prev_glyph_id = Some(glyph_id);

        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };

        let bounds = outlined.px_bounds();
        outlined.draw(|gx, gy, coverage| {
            let px = bounds.min.x as i32 + gx as i32;
            let py = bounds.min.y as i32 + gy as i32;
            if px < 0 || py < 0 || px >= img.width() as i32 || py >= img.height() as i32 {
                return;
            }
            let dst = img.get_pixel_mut(px as u32, py as u32);
            *dst = simple_blend(dst, color, coverage);
        });
    }
}

/// Alpha-blend `src` (with an additional `coverage` factor from the
/// glyph rasteriser) over the existing destination pixel.
#[inline]
fn simple_blend(dst: &Rgba<u8>, src: Rgba<u8>, coverage: f32) -> Rgba<u8> {
    let src_a = (src[3] as f32 / 255.0) * coverage;
    let dst_a = dst[3] as f32 / 255.0;

    // "alpha over" composite
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a < f32::EPSILON {
        return Rgba([0, 0, 0, 0]);
    }

    let blend = |s: u8, d: u8| -> u8 {
        let val = (s as f32 * src_a + d as f32 * dst_a * (1.0 - src_a)) / out_a;
        val.round() as u8
    };

    Rgba([
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0).round() as u8,
    ])
}

/// Generate a ring of (dx, dy) offsets for stroke rendering.
///
/// Eight evenly-spaced points on a circle of radius `width` give a
/// good approximation of a stroke outline at typical font sizes.
fn generate_stroke_offsets(width: f32) -> Vec<(f32, f32)> {
    if width <= 0.0 {
        return vec![];
    }
    // Use more samples for larger strokes to avoid obvious gaps.
    let steps = if width <= 2.0 { 8 } else { 16 };
    (0..steps)
        .map(|i| {
            let angle = std::f32::consts::TAU * (i as f32) / (steps as f32);
            (width * angle.cos(), width * angle.sin())
        })
        .collect()
}
