//! Rasterise a [`TextLayer`] into an RGBA image buffer.
//!
//! The renderer uses `ab_glyph` for glyph layout and coverage maps.
//! Stroke outlines are produced by drawing the text at a ring of
//! offset positions (with the stroke colour) before drawing the fill
//! text on top.  The output is a tight bounding-box image sized to
//! exactly the rendered text plus a [`stroke_pad`] margin on every side
//! for stroke overflow; positioning onto a frame canvas is the
//! compositor's responsibility (see [`stroke_pad`] for the anchoring
//! contract).

use std::sync::{Arc, Mutex, OnceLock};

use ab_glyph::{Font, FontArc, Glyph, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};

use crate::error::AppError;
use crate::fonts::load_font;
use crate::layer::TextLayer;

// ---------------------------------------------------------------------------
// Render cache
// ---------------------------------------------------------------------------

/// Cache key covering every field of [`TextLayer`] that affects the
/// rasterised pixels.  Transform fields (position, scale, skew, rotation,
/// opacity, keyframes) are applied by the compositor AFTER rasterisation
/// and deliberately excluded.  `f64` fields are keyed by bit pattern so
/// NaN/−0.0 edge cases stay well-defined.
///
/// Keep in sync with `textRasterKey` in `src/lib/utils/canvas-renderer.ts`:
/// both keys must cover the same set of rasterisation-affecting fields.
type RenderCacheKey = (
    String,                 // text
    String,                 // font_family
    u64,                    // font_size.to_bits()
    [u8; 4],                // color
    Option<([u8; 4], u64)>, // stroke (color, width.to_bits())
    String,                 // text_align
    Option<u64>,            // max_width.map(f64::to_bits)
);

/// Maximum number of rasterised text images retained.  Small and capped so
/// the cache cannot grow without bound; entries are evicted LRU-first.
const RENDER_CACHE_CAPACITY: usize = 16;

/// Cache storage: (key, image) entries ordered most-recently-used first.
/// A Vec is used instead of a HashMap because the capacity is tiny (16):
/// a linear key scan is cheaper than hashing and keeps true LRU ordering
/// trivial.
type RenderCacheEntries = Vec<(RenderCacheKey, Arc<RgbaImage>)>;

/// LRU cache of rasterised text images, most-recently-used first.
fn render_cache() -> &'static Mutex<RenderCacheEntries> {
    static CACHE: OnceLock<Mutex<RenderCacheEntries>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Vec::with_capacity(RENDER_CACHE_CAPACITY)))
}

/// Empty the process-wide render cache.
///
/// Test-only hook: integration tests call this so `Arc::ptr_eq` cache-hit
/// assertions start from a known-empty cache instead of racing other tests
/// for the 16 LRU slots.  `#[doc(hidden)] pub` rather than `pub(crate)`
/// because integration tests link the library from outside the crate.
#[doc(hidden)]
pub fn clear_render_cache() {
    render_cache().lock().unwrap().clear();
}

fn render_cache_key(layer: &TextLayer) -> RenderCacheKey {
    (
        layer.text.clone(),
        layer.font_family.clone(),
        layer.font_size.to_bits(),
        layer.color,
        layer.stroke.as_ref().map(|s| (s.color, s.width.to_bits())),
        layer.text_align.clone(),
        layer.max_width.map(f64::to_bits),
    )
}

/// Padding, in pixels, reserved on every side of the glyph box for
/// stroke overflow.
///
/// The rendered image places the glyph box origin at `(pad, pad)`, so
/// callers that anchor the image top-left at the layer position must
/// offset placement by `-pad` on both axes for the glyph box (not the
/// pad edge) to land on the layer position — matching the preview,
/// which draws text at the transform origin with the stroke
/// overflowing beyond it.
pub fn stroke_pad(layer: &TextLayer) -> u32 {
    layer
        .stroke
        .as_ref()
        .map(|s| s.width.ceil() as u32 + 2)
        .unwrap_or(0)
}

/// Convert a CSS-pixel em size (what `ctx.font = "48px ..."` means in
/// the preview) to the `PxScale` ab_glyph expects.
///
/// `PxScale` is the ascent−descent height in pixels, which for most
/// fonts is larger than one em.  Scaling by
/// `height_unscaled / units_per_em` makes `font_size` mean "px per em",
/// mirroring `Font::pt_to_px_scale` without the pt→px factor.
fn css_px_scale(font: &FontArc, font_size: f32) -> Result<PxScale, AppError> {
    let units_per_em = font
        .units_per_em()
        .ok_or_else(|| AppError::Font("font has an invalid units_per_em".to_string()))?;
    Ok(PxScale::from(
        font_size * font.height_unscaled() / units_per_em,
    ))
}

/// Render `layer` to an RGBA image.
///
/// Returns a 1×1 transparent image for empty text so that callers
/// never have to handle a zero-dimension surface.
///
/// Results are memoised in a small LRU cache keyed by every
/// content-affecting field, so compositing the same text layer across many
/// frames rasterises it once and hands out cheap [`Arc`] clones; only the
/// per-frame position/opacity (applied later by the compositor) vary.
pub fn render_text(layer: &TextLayer) -> Result<Arc<RgbaImage>, AppError> {
    if layer.text.is_empty() {
        return Ok(Arc::new(RgbaImage::new(1, 1)));
    }

    let key = render_cache_key(layer);
    {
        let mut cache = render_cache().lock().unwrap();
        if let Some(pos) = cache.iter().position(|(k, _)| *k == key) {
            // Move the hit to the front (most recently used).
            let entry = cache.remove(pos);
            let img = Arc::clone(&entry.1);
            cache.insert(0, entry);
            return Ok(img);
        }
    }

    // Rasterise outside the lock so concurrent renders never serialise on
    // glyph drawing (worst case: two threads render the same key once each).
    let img = Arc::new(render_text_uncached(layer)?);

    let mut cache = render_cache().lock().unwrap();
    if !cache.iter().any(|(k, _)| *k == key) {
        cache.insert(0, (key, Arc::clone(&img)));
        cache.truncate(RENDER_CACHE_CAPACITY);
    }
    Ok(img)
}

/// Rasterise `layer` without consulting the cache.
fn render_text_uncached(layer: &TextLayer) -> Result<RgbaImage, AppError> {
    let font = load_font(&layer.font_family)?;
    // Interpret font_size as a CSS-px em size, matching the preview's
    // `ctx.font = `${fontSize}px ...``.
    let scale = css_px_scale(&font, layer.font_size as f32)?;

    // Named `pad` (mirroring the TS renderer) to avoid shadowing the
    // `stroke_pad` fn it is computed from.
    let pad = stroke_pad(layer);

    // Wrap text into lines.
    let lines = wrap_text(&font, &layer.text, scale, layer.max_width);
    if lines.is_empty() {
        return Ok(RgbaImage::new(1, 1));
    }

    let scaled = font.as_scaled(scale);
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    // Match the preview's line advance (canvas-renderer.ts):
    // fontSize * 1.2 CSS px per line.
    let line_height = layer.font_size as f32 * 1.2;

    // Measure each line.
    let line_widths: Vec<u32> = lines
        .iter()
        .map(|l| measure_text_width(&font, l, scale))
        .collect();

    let canvas_w = if let Some(mw) = layer.max_width {
        mw.ceil() as u32 + pad * 2
    } else {
        *line_widths.iter().max().unwrap_or(&1) + pad * 2
    };
    // Tall enough for the last line's full ascent+descent even when it
    // exceeds the 1.2 line advance (e.g. Anton is ~1.5 em tall).
    let text_block_h = (lines.len() - 1) as f32 * line_height + (ascent - descent);
    let canvas_h = text_block_h.ceil() as u32 + pad * 2;

    let mut img = RgbaImage::new(canvas_w.max(1), canvas_h.max(1));
    let pad = pad as f32;

    for (i, line) in lines.iter().enumerate() {
        let lw = line_widths[i] as f32;
        let content_w = if let Some(mw) = layer.max_width {
            mw as f32
        } else {
            lw
        };
        let x0 = match layer.text_align.as_str() {
            "center" => pad + (content_w - lw) / 2.0,
            "right" => pad + content_w - lw,
            _ => pad, // "left" default
        };
        // The preview draws with textBaseline='top' at y = i * lineHeight;
        // the em-box top sits `ascent` above the baseline, so placing the
        // baseline at y + ascent aligns the em-box top with the preview's y.
        let y0 = pad + i as f32 * line_height + ascent;

        if let Some(ref stroke) = layer.stroke {
            let offsets = generate_stroke_offsets(stroke.width as f32);
            let stroke_color = Rgba(stroke.color);
            for (dx, dy) in &offsets {
                draw_text_at(&mut img, &font, line, scale, x0 + dx, y0 + dy, stroke_color);
            }
        }

        let fill_color = Rgba(layer.color);
        draw_text_at(&mut img, &font, line, scale, x0, y0, fill_color);
    }

    Ok(img)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Measure the pixel width of `text` at `scale`.
fn measure_text_width(font: &FontArc, text: &str, scale: PxScale) -> u32 {
    let scaled = font.as_scaled(scale);
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
    width.ceil() as u32
}

/// Split `text` into lines that each fit within `max_width` pixels.
/// If `max_width` is None, returns a single line (the full text).
fn wrap_text(font: &FontArc, text: &str, scale: PxScale, max_width: Option<f64>) -> Vec<String> {
    let Some(max_w) = max_width else {
        return vec![text.to_string()];
    };
    let max_w = max_w as f32;
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let candidate = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{current_line} {word}")
        };
        let w = measure_text_width(font, &candidate, scale);
        if w as f32 <= max_w || current_line.is_empty() {
            current_line = candidate;
        } else {
            // Current line is full; push it and start a new one.
            if !current_line.is_empty() {
                lines.push(current_line.clone());
            }
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
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
