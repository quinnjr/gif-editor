use image::{imageops, Rgba, RgbaImage};

use crate::layer::Layer;

/// Composite all visible, in-range layers onto a clone of `base` for the
/// given `frame_index`.  Returns a new image; the base is not mutated.
pub fn composite_frame(base: &RgbaImage, layers: &[Layer], frame_index: usize) -> RgbaImage {
    let mut target = base.clone();

    for layer in layers {
        // Skip invisible layers.
        if !layer.visible() {
            continue;
        }

        // frame_range is inclusive on both ends; (0,0) means frame 0 only.
        let (start, end) = layer.frame_range();
        if frame_index < start || frame_index > end {
            continue;
        }

        match layer {
            Layer::Image(img_layer) => {
                let Some(ref src) = img_layer.image_data else {
                    continue;
                };

                // Scale the source image if needed.
                let scaled: RgbaImage = if (img_layer.scale - 1.0).abs() > f64::EPSILON {
                    let new_w = ((src.width() as f64) * img_layer.scale).round() as u32;
                    let new_h = ((src.height() as f64) * img_layer.scale).round() as u32;
                    if new_w == 0 || new_h == 0 {
                        continue;
                    }
                    imageops::resize(src, new_w, new_h, imageops::FilterType::Lanczos3)
                } else {
                    src.clone()
                };

                composite_rgba_buffer(
                    &mut target,
                    &scaled,
                    img_layer.position,
                    img_layer.opacity,
                );
            }
            Layer::Text(text_layer) => {
                if let Ok(text_img) = crate::text_renderer::render_text(text_layer) {
                    composite_rgba_buffer(
                        &mut target,
                        &text_img,
                        text_layer.position,
                        text_layer.opacity,
                    );
                }
            }
        }
    }

    target
}

/// Blend `buffer` onto `target` at `position` with the given `opacity`
/// (0.0 = fully transparent, 1.0 = fully opaque).
///
/// This is also the entry point used by the text renderer (Task 6) to
/// blend rasterised glyph bitmaps onto a frame.
pub fn composite_rgba_buffer(
    target: &mut RgbaImage,
    buffer: &RgbaImage,
    position: (f64, f64),
    opacity: f64,
) {
    let (tw, th) = target.dimensions();
    let off_x = position.0.round() as i64;
    let off_y = position.1.round() as i64;

    for (bx, by, src_pixel) in buffer.enumerate_pixels() {
        let tx = off_x + bx as i64;
        let ty = off_y + by as i64;

        // Skip pixels that fall outside the target canvas.
        if tx < 0 || ty < 0 || tx >= tw as i64 || ty >= th as i64 {
            continue;
        }

        let dst_pixel = target.get_pixel_mut(tx as u32, ty as u32);
        let effective_alpha = (src_pixel[3] as f64 / 255.0) * opacity;
        *dst_pixel = alpha_blend(dst_pixel, src_pixel, effective_alpha);
    }
}

/// Standard "alpha over" composite.
///
/// `src_alpha` is the pre-multiplied effective alpha in [0.0, 1.0]; the
/// destination pixel's own alpha channel is assumed to be 1.0 (fully
/// opaque base frame).
fn alpha_blend(dst: &Rgba<u8>, src: &Rgba<u8>, src_alpha: f64) -> Rgba<u8> {
    let inv = 1.0 - src_alpha;
    let r = (src[0] as f64 * src_alpha + dst[0] as f64 * inv).round() as u8;
    let g = (src[1] as f64 * src_alpha + dst[1] as f64 * inv).round() as u8;
    let b = (src[2] as f64 * src_alpha + dst[2] as f64 * inv).round() as u8;
    // Keep destination alpha (base frames are always opaque).
    Rgba([r, g, b, dst[3]])
}
