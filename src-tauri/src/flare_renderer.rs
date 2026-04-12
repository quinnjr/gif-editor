use crate::layer::FlareLayer;
use image::RgbaImage;

// ---------------------------------------------------------------------------
// Ghost artifact constants — declared at module scope so they are available
// to render_flare without paying a stack-allocation cost on every call.
// ---------------------------------------------------------------------------

/// Fractional offsets along the lens axis for each ghost artifact.
const GHOST_OFFSETS: [f64; 4] = [0.3, 0.6, 1.0, 1.4];
/// Relative size multipliers for each ghost artifact.
const GHOST_SIZES: [f64; 4] = [0.3, 0.2, 0.4, 0.15];
/// Base brightness multipliers for each ghost artifact.
const GHOST_ALPHAS: [f64; 4] = [0.6, 0.4, 0.7, 0.3];

// ---------------------------------------------------------------------------
// Primitive helpers — all use additive blending within the flare canvas
// ---------------------------------------------------------------------------

/// Additively blend `color` at `alpha` intensity onto pixel `(x, y)`.
///
/// RGB channels are scaled by `alpha/255` so a fully transparent contribution
/// adds nothing.  The alpha channel uses the same saturating-u32 pattern so
/// all four channels are accumulated consistently.
fn add_pixel(img: &mut RgbaImage, x: u32, y: u32, color: [u8; 3], alpha: u8) {
    if x >= img.width() || y >= img.height() {
        return;
    }
    let p = img.get_pixel_mut(x, y);
    let a = alpha as u32;
    p[0] = ((p[0] as u32 + color[0] as u32 * a / 255).min(255)) as u8;
    p[1] = ((p[1] as u32 + color[1] as u32 * a / 255).min(255)) as u8;
    p[2] = ((p[2] as u32 + color[2] as u32 * a / 255).min(255)) as u8;
    // Alpha accumulates coverage with the same saturating-u32 + min(255)
    // pattern as the RGB channels, rather than a raw wrapping `+= a`.
    p[3] = ((p[3] as u32 + a).min(255)) as u8;
}

/// Radial glow with quadratic falloff: alpha = (1 - d/radius)² × brightness.
fn draw_glow(
    img: &mut RgbaImage,
    cx: f64,
    cy: f64,
    radius: f64,
    color: [u8; 3],
    brightness: f64,
) {
    if radius < 1.0 {
        return;
    }
    let x0 = ((cx - radius).floor() as i32).max(0) as u32;
    let y0 = ((cy - radius).floor() as i32).max(0) as u32;
    let x1 = ((cx + radius).ceil() as i32).min(img.width() as i32 - 1) as u32;
    let y1 = ((cy + radius).ceil() as i32).min(img.height() as i32 - 1) as u32;

    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d < radius {
                let t = 1.0 - d / radius;
                let alpha = (t * t * brightness * 255.0).clamp(0.0, 255.0) as u8;
                add_pixel(img, x, y, color, alpha);
            }
        }
    }
}

/// 8 thin starburst streaks radiating from `(cx, cy)`, `length` pixels long.
fn draw_starburst(img: &mut RgbaImage, cx: f64, cy: f64, length: f64, brightness: f64) {
    // Bound iteration to the square that inscribes the spoke circle, matching
    // the pattern used by draw_glow and draw_ring.  Skipping the rest of the
    // canvas avoids an O(W×H) walk for every frame.
    let x0 = ((cx - length).floor() as i32).max(0) as u32;
    let y0 = ((cy - length).floor() as i32).max(0) as u32;
    let x1 = ((cx + length).ceil() as i32).min(img.width() as i32 - 1) as u32;
    let y1 = ((cy + length).ceil() as i32).min(img.height() as i32 - 1) as u32;

    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d < 2.0 || d > length {
                continue;
            }
            let angle = dy.atan2(dx);
            // Fold the full [0, 2π) circle onto [0, π) so that opposite spokes
            // share the same angular bucket.  With 8 spokes evenly spaced over
            // the half-circle, each bucket is π/8 wide.
            let angle_mod = angle.rem_euclid(std::f64::consts::PI);
            // min_angular_dist is the angular distance (in radians) to the
            // nearest spoke axis.  Multiplying by `d` converts it to pixels
            // perpendicular to that axis, giving a width test that is constant
            // in pixel space regardless of distance from the centre.
            let min_angular_dist = (0u32..8)
                .map(|k| {
                    let sa = k as f64 * std::f64::consts::PI / 8.0;
                    let diff = (angle_mod - sa).abs();
                    diff.min(std::f64::consts::PI - diff)
                })
                .fold(f64::MAX, f64::min);

            let streak_width_px = (1.5 + 2.0 * (1.0 - d / length)).max(0.5);
            let pixel_perp = min_angular_dist * d;
            if pixel_perp < streak_width_px {
                let t_len = 1.0 - d / length;
                let t_width = 1.0 - pixel_perp / streak_width_px;
                let alpha =
                    (t_len * t_width * brightness * 0.6 * 255.0).clamp(0.0, 255.0) as u8;
                add_pixel(img, x, y, [255, 255, 255], alpha);
            }
        }
    }
}

/// Soft ring (annulus) centred at `(cx, cy)`.
fn draw_ring(
    img: &mut RgbaImage,
    cx: f64,
    cy: f64,
    radius: f64,
    thickness: f64,
    color: [u8; 3],
    brightness: f64,
) {
    let outer = radius + thickness;
    let x0 = ((cx - outer).floor() as i32).max(0) as u32;
    let y0 = ((cy - outer).floor() as i32).max(0) as u32;
    let x1 = ((cx + outer).ceil() as i32).min(img.width() as i32 - 1) as u32;
    let y1 = ((cy + outer).ceil() as i32).min(img.height() as i32 - 1) as u32;

    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let dist_from_ring = (d - radius).abs();
            if dist_from_ring < thickness {
                let t = 1.0 - dist_from_ring / thickness;
                let alpha = (t * t * brightness * 180.0).clamp(0.0, 255.0) as u8;
                add_pixel(img, x, y, color, alpha);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a full-canvas lens flare image for `layer` at `position` on frame
/// `frame_index`.  Returns a fresh RGBA image the size of the canvas;
/// the compositor should blend this additively onto the base frame.
pub fn render_flare(
    layer: &FlareLayer,
    position: (f64, f64),
    frame_index: usize,
    canvas_width: u32,
    canvas_height: u32,
) -> RgbaImage {
    let mut img = RgbaImage::new(canvas_width, canvas_height);
    let (ox, oy) = position;

    // Pulsing brightness multiplier
    let brightness =
        layer.intensity * (1.0 + 0.3 * (frame_index as f64 * layer.pulse_speed).sin());

    // 1. Central white glow
    draw_glow(&mut img, ox, oy, layer.scale * 80.0, [255, 255, 255], brightness);

    // 2. Starburst (8 streaks, white)
    draw_starburst(&mut img, ox, oy, layer.scale * 200.0, brightness);

    // 3. Yellow inner ring + soft orange outer halo
    draw_ring(
        &mut img,
        ox,
        oy,
        layer.scale * 100.0,
        layer.scale * 25.0,
        [0xFF, 0xE8, 0x7C], // #FFE87C yellow
        brightness,
    );
    draw_glow(
        &mut img,
        ox,
        oy,
        layer.scale * 140.0,
        [0xFF, 0x7B, 0x00], // #FF7B00 orange
        brightness * 0.4,
    );

    // 4. Blue ghost artifacts along axis from origin through canvas centre
    let ccx = canvas_width as f64 / 2.0;
    let ccy = canvas_height as f64 / 2.0;
    let axis_x = ccx - ox;
    let axis_y = ccy - oy;

    for i in 0..4 {
        let phase = i as f64 * 0.5;
        let gb = brightness
            * GHOST_ALPHAS[i]
            * (1.0 + 0.3 * (frame_index as f64 * layer.pulse_speed + phase).sin());
        let gx = ox + axis_x * GHOST_OFFSETS[i];
        let gy = oy + axis_y * GHOST_OFFSETS[i];
        let gr = layer.scale * 80.0 * GHOST_SIZES[i];
        draw_glow(&mut img, gx, gy, gr, [0x4B, 0x6E, 0xAF], gb); // #4B6EAF blue
    }

    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_flare_returns_correct_dimensions() {
        let layer = FlareLayer::new();
        let img = render_flare(&layer, (50.0, 50.0), 0, 100, 80);
        assert_eq!(img.width(), 100);
        assert_eq!(img.height(), 80);
    }

    #[test]
    fn render_flare_center_pixel_is_nonzero_at_high_intensity() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let img = render_flare(&layer, (50.0, 50.0), 0, 100, 100);
        let p = img.get_pixel(50, 50);
        assert!(p[3] > 0, "center pixel alpha should be > 0; got {}", p[3]);
    }

    /// The central glow is white, so all three colour channels must be lit up
    /// near the flare origin — not just the alpha channel.
    #[test]
    fn render_flare_center_region_has_nonzero_rgb() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let img = render_flare(&layer, (50.0, 50.0), 0, 100, 100);
        let p = img.get_pixel(50, 50);
        assert!(p[0] > 0, "center pixel R should be > 0; got {}", p[0]);
        assert!(p[1] > 0, "center pixel G should be > 0; got {}", p[1]);
        assert!(p[2] > 0, "center pixel B should be > 0; got {}", p[2]);
    }

    /// Pixels in the far corner of a small canvas should be unaffected by a
    /// normally-scaled flare positioned at the centre.  At scale=1.0 the
    /// largest element (starburst) reaches scale*200 = 200 px; on a 100×100
    /// canvas the flare is at (50,50) so the corner is only ~70 px away and
    /// could legitimately be lit.  Use a tiny scale so nothing reaches the
    /// corner.
    #[test]
    fn render_flare_far_corner_is_dark_at_small_scale() {
        let mut layer = FlareLayer::new();
        layer.intensity = 1.0;
        layer.scale = 0.05; // largest element reaches only 0.05*200 = 10 px
        // 60×60 canvas, flare at centre (30,30); corner (0,0) is ~42 px away
        let img = render_flare(&layer, (30.0, 30.0), 0, 60, 60);
        let corner = img.get_pixel(0, 0);
        assert!(
            corner[3] < 10,
            "corner pixel alpha should be near-zero for small-scale flare; got {}",
            corner[3]
        );
    }
}
