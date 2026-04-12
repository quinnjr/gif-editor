use image::{Rgba, RgbaImage};

use crate::layer::{Layer, interpolate_keyframes};

/// Affine transform parameters for a layer.
struct AffineParams {
    position: (f64, f64),
    scale_x: f64,
    scale_y: f64,
    skew_x: f64,
    skew_y: f64,
    rotation: f64, // degrees
    opacity: f64,
}

/// Composite all visible, in-range layers onto a clone of `base` for the
/// given `frame_index`.  Returns a new image; the base is not mutated.
pub fn composite_frame(base: &RgbaImage, layers: &[Layer], frame_index: usize) -> RgbaImage {
    let mut target = base.clone();

    for layer in layers {
        if !layer.visible() {
            continue;
        }

        let (start, end) = layer.frame_range();
        if frame_index < start || frame_index > end {
            continue;
        }

        match layer {
            Layer::Image(img_layer) => {
                // For animated GIF overlays, pick the frame that
                // corresponds to the current project frame (looping).
                let src: &RgbaImage = if !img_layer.frames.is_empty() {
                    let offset = frame_index.saturating_sub(start);
                    &img_layer.frames[offset % img_layer.frames.len()]
                } else if let Some(ref img) = img_layer.image_data {
                    img
                } else {
                    continue;
                };

                let sx = img_layer.scale_x;
                let sy = img_layer.scale_y;
                let kx = img_layer.skew_x;
                let ky = img_layer.skew_y;

                let (pos, opacity) = match interpolate_keyframes(&img_layer.keyframes, frame_index)
                {
                    Some((p, o)) => (p, o),
                    None => (img_layer.position, img_layer.opacity),
                };

                let params = AffineParams {
                    position: pos,
                    scale_x: sx,
                    scale_y: sy,
                    skew_x: kx,
                    skew_y: ky,
                    rotation: img_layer.rotation,
                    opacity,
                };
                if is_identity(sx, sy, kx, ky, img_layer.rotation) {
                    composite_rgba_buffer(&mut target, src, pos, opacity);
                } else {
                    affine_composite(&mut target, src, &params);
                }
            }
            Layer::Text(text_layer) => {
                if let Ok(text_img) = crate::text_renderer::render_text(text_layer) {
                    let sx = text_layer.scale_x;
                    let sy = text_layer.scale_y;
                    let kx = text_layer.skew_x;
                    let ky = text_layer.skew_y;

                    let (pos, opacity) =
                        match interpolate_keyframes(&text_layer.keyframes, frame_index) {
                            Some((p, o)) => (p, o),
                            None => (text_layer.position, text_layer.opacity),
                        };

                    let params = AffineParams {
                        position: pos,
                        scale_x: sx,
                        scale_y: sy,
                        skew_x: kx,
                        skew_y: ky,
                        rotation: text_layer.rotation,
                        opacity,
                    };
                    if is_identity(sx, sy, kx, ky, text_layer.rotation) {
                        composite_rgba_buffer(&mut target, &text_img, pos, opacity);
                    } else {
                        affine_composite(&mut target, &text_img, &params);
                    }
                }
            }
            // Flare layers are composited additively with no affine transform;
            // render_flare() places all elements at canvas coordinates directly.
            Layer::Flare(flare_layer) => {
                let (pos, opacity) =
                    match interpolate_keyframes(&flare_layer.keyframes, frame_index) {
                        Some((p, o)) => (p, o),
                        None => (flare_layer.position, flare_layer.opacity),
                    };
                let flare_img = crate::flare_renderer::render_flare(
                    flare_layer,
                    pos,
                    frame_index,
                    target.width(),
                    target.height(),
                );
                debug_assert_eq!(
                    (flare_img.width(), flare_img.height()),
                    (target.width(), target.height()),
                    "render_flare must return a canvas-sized image"
                );
                additive_composite(&mut target, &flare_img, opacity);
            }
        }
    }

    target
}

/// Returns `true` when the 2×2 transform portion is the identity matrix.
fn is_identity(sx: f64, sy: f64, kx: f64, ky: f64, rotation: f64) -> bool {
    (sx - 1.0).abs() < f64::EPSILON
        && (sy - 1.0).abs() < f64::EPSILON
        && kx.abs() < f64::EPSILON
        && ky.abs() < f64::EPSILON
        && rotation.abs() < f64::EPSILON
}

/// Composite `src` onto `target` using the affine transform defined by
/// combined rotation × scale/skew matrix with translation `position`.
///
/// The matrix maps source → destination:
///
/// ```text
///   dst_x = a * src_x + c * src_y + tx
///   dst_y = b * src_x + d * src_y + ty
/// ```
///
/// where (a,b,c,d) = rotation × scale/skew matrix.
///
/// We iterate over the output bounding box and use the inverse matrix to
/// sample source pixels with bilinear interpolation.
fn affine_composite(target: &mut RgbaImage, src: &RgbaImage, params: &AffineParams) {
    let (tw, th) = (target.width() as i64, target.height() as i64);
    let (sw, sh) = (src.width() as f64, src.height() as f64);
    let (tx, ty) = params.position;
    let (sx, sy, kx, ky) = (params.scale_x, params.scale_y, params.skew_x, params.skew_y);
    let theta = params.rotation.to_radians();
    let (cos_t, sin_t) = (theta.cos(), theta.sin());
    let opacity = params.opacity;

    // Combined rotation × scale/skew matrix:
    //   a = cos*sx - sin*ky,  c = cos*kx - sin*sy
    //   b = sin*sx + cos*ky,  d = sin*kx + cos*sy
    // dst_x = a*src_x + c*src_y + tx
    // dst_y = b*src_x + d*src_y + ty
    let a = cos_t * sx - sin_t * ky;
    let b = sin_t * sx + cos_t * ky;
    let c = cos_t * kx - sin_t * sy;
    let d = sin_t * kx + cos_t * sy;

    // Compute output bounding box by mapping all four source corners.
    let corners = [(0.0, 0.0), (sw, 0.0), (0.0, sh), (sw, sh)];
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for (cx, cy) in &corners {
        let dx = a * cx + c * cy + tx;
        let dy = b * cx + d * cy + ty;
        min_x = min_x.min(dx);
        min_y = min_y.min(dy);
        max_x = max_x.max(dx);
        max_y = max_y.max(dy);
    }

    let x0 = (min_x.floor() as i64).max(0);
    let y0 = (min_y.floor() as i64).max(0);
    let x1 = (max_x.ceil() as i64).min(tw);
    let y1 = (max_y.ceil() as i64).min(th);

    let det = a * d - c * b;
    if det.abs() < 1e-12 {
        return;
    }
    let inv_a = d / det;
    let inv_b = -b / det;
    let inv_c = -c / det;
    let inv_d = a / det;

    for dst_y in y0..y1 {
        for dst_x in x0..x1 {
            let rx = dst_x as f64 - tx;
            let ry = dst_y as f64 - ty;
            let src_xf = inv_a * rx + inv_c * ry;
            let src_yf = inv_b * rx + inv_d * ry;

            if src_xf < -0.5 || src_yf < -0.5 || src_xf > sw - 0.5 || src_yf > sh - 0.5 {
                continue;
            }

            let src_pixel = bilinear_sample(src, src_xf, src_yf);
            if src_pixel[3] == 0 {
                continue;
            }

            let dst_pixel = target.get_pixel_mut(dst_x as u32, dst_y as u32);
            let effective_alpha = (src_pixel[3] as f64 / 255.0) * opacity;
            *dst_pixel = alpha_blend(dst_pixel, &src_pixel, effective_alpha);
        }
    }
}

/// Bilinear sampling from an RGBA image at fractional coordinates.
fn bilinear_sample(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x = x.max(0.0).min(w - 1.0);
    let y = y.max(0.0).min(h - 1.0);

    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);

    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let p00 = img.get_pixel(x0, y0);
    let p10 = img.get_pixel(x1, y0);
    let p01 = img.get_pixel(x0, y1);
    let p11 = img.get_pixel(x1, y1);

    let lerp = |a: u8, b: u8, c: u8, d: u8| -> u8 {
        let top = a as f64 * (1.0 - fx) + b as f64 * fx;
        let bot = c as f64 * (1.0 - fx) + d as f64 * fx;
        let val = top * (1.0 - fy) + bot * fy;
        val.round() as u8
    };

    Rgba([
        lerp(p00[0], p10[0], p01[0], p11[0]),
        lerp(p00[1], p10[1], p01[1], p11[1]),
        lerp(p00[2], p10[2], p01[2], p11[2]),
        lerp(p00[3], p10[3], p01[3], p11[3]),
    ])
}

/// Blend `buffer` onto `target` at `position` with the given `opacity`.
/// Fast path for identity transforms (no scale, no skew).
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

        if tx < 0 || ty < 0 || tx >= tw as i64 || ty >= th as i64 {
            continue;
        }

        let dst_pixel = target.get_pixel_mut(tx as u32, ty as u32);
        let effective_alpha = (src_pixel[3] as f64 / 255.0) * opacity;
        *dst_pixel = alpha_blend(dst_pixel, src_pixel, effective_alpha);
    }
}

/// Additively blend `src` onto `target` — each channel is clamped at 255.
/// Used for light-emitting effects (lens flares) where the correct model
/// is "add light" rather than "paint over".
// Note: iterates the full canvas; a future optimisation could pass a bounding
// box from render_flare() to skip the many zero-alpha pixels outside the flare.
pub(crate) fn additive_composite(target: &mut RgbaImage, src: &RgbaImage, opacity: f64) {
    let (tw, th) = target.dimensions();
    for y in 0..th {
        for x in 0..tw {
            let src_pixel = src.get_pixel(x, y);
            if src_pixel[3] == 0 {
                continue;
            }
            let dst_pixel = target.get_pixel_mut(x, y);
            let eff = (src_pixel[3] as f64 / 255.0) * opacity;
            dst_pixel[0] =
                ((dst_pixel[0] as f64 + src_pixel[0] as f64 * eff).min(255.0)) as u8;
            dst_pixel[1] =
                ((dst_pixel[1] as f64 + src_pixel[1] as f64 * eff).min(255.0)) as u8;
            dst_pixel[2] =
                ((dst_pixel[2] as f64 + src_pixel[2] as f64 * eff).min(255.0)) as u8;
            // dst alpha is preserved — we are adding light, not covering pixels
        }
    }
}

/// Standard "alpha over" composite.
fn alpha_blend(dst: &Rgba<u8>, src: &Rgba<u8>, src_alpha: f64) -> Rgba<u8> {
    let inv = 1.0 - src_alpha;
    let r = (src[0] as f64 * src_alpha + dst[0] as f64 * inv).round() as u8;
    let g = (src[1] as f64 * src_alpha + dst[1] as f64 * inv).round() as u8;
    let b = (src[2] as f64 * src_alpha + dst[2] as f64 * inv).round() as u8;
    Rgba([r, g, b, dst[3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn make_image(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for pixel in img.pixels_mut() {
            *pixel = Rgba([r, g, b, a]);
        }
        img
    }

    #[test]
    fn additive_composite_adds_channels_clamped() {
        // target: solid dark grey (100, 100, 100, 255)
        // src:    solid white (255, 255, 255, 255) at opacity 1.0
        // expected: each channel clamps to 255
        let mut target = make_image(4, 4, 100, 100, 100, 255);
        let src = make_image(4, 4, 255, 255, 255, 255);
        additive_composite(&mut target, &src, 1.0);
        let p = *target.get_pixel(0, 0);
        assert_eq!(p[0], 255);
        assert_eq!(p[1], 255);
        assert_eq!(p[2], 255);
        assert_eq!(p[3], 255, "dst alpha must be preserved");
    }

    #[test]
    fn additive_composite_skips_zero_alpha_pixels() {
        let mut target = make_image(4, 4, 50, 50, 50, 255);
        let src = make_image(4, 4, 255, 255, 255, 0); // fully transparent src
        additive_composite(&mut target, &src, 1.0);
        let p = *target.get_pixel(0, 0);
        assert_eq!(p[0], 50, "zero-alpha pixels must not change target");
    }

    #[test]
    fn additive_composite_respects_opacity() {
        // target: 0 everywhere
        // src:    255 channels, alpha 255, opacity 0.5
        // expected: each channel ≈ 127 (255 * 1.0 * 0.5)
        let mut target = make_image(4, 4, 0, 0, 0, 255);
        let src = make_image(4, 4, 255, 255, 255, 255);
        additive_composite(&mut target, &src, 0.5);
        let p = *target.get_pixel(0, 0);
        assert!(p[0] >= 126 && p[0] <= 128, "expected ~127, got {}", p[0]);
    }
}
