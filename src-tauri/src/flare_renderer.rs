use crate::layer::FlareLayer;
use image::RgbaImage;

// ---------------------------------------------------------------------------
// Primitive helpers — all use additive blending within the flare canvas
// ---------------------------------------------------------------------------

/// Additively blend `color` at `alpha` intensity onto pixel `(x, y)`.
fn add_pixel(img: &mut RgbaImage, x: u32, y: u32, color: [u8; 3], alpha: u8) {
    if x >= img.width() || y >= img.height() {
        return;
    }
    let p = img.get_pixel_mut(x, y);
    let a = alpha as u32;
    p[0] = ((p[0] as u32 + color[0] as u32 * a / 255).min(255)) as u8;
    p[1] = ((p[1] as u32 + color[1] as u32 * a / 255).min(255)) as u8;
    p[2] = ((p[2] as u32 + color[2] as u32 * a / 255).min(255)) as u8;
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
    for y in 0..img.height() {
        for x in 0..img.width() {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d < 2.0 || d > length {
                continue;
            }
            let angle = dy.atan2(dx);
            let angle_mod = angle.rem_euclid(std::f64::consts::PI);
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

    const OFFSETS: [f64; 4] = [0.3, 0.6, 1.0, 1.4];
    const SIZES: [f64; 4] = [0.3, 0.2, 0.4, 0.15];
    const ALPHAS: [f64; 4] = [0.6, 0.4, 0.7, 0.3];

    for i in 0..4 {
        let phase = i as f64 * 0.5;
        let gb = brightness
            * ALPHAS[i]
            * (1.0 + 0.3 * (frame_index as f64 * layer.pulse_speed + phase).sin());
        let gx = ox + axis_x * OFFSETS[i];
        let gy = oy + axis_y * OFFSETS[i];
        let gr = layer.scale * 80.0 * SIZES[i];
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
}
