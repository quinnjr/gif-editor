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

/// The 8 starburst spoke axis angles, folded onto `[0, π)`: `k·π/8` for
/// `k = 0..8`.  Precomputed once so the per-pixel loop in
/// [`draw_starburst`] indexes a table instead of recomputing each angle.
const SPOKE_AXES: [f64; 8] = {
    let mut axes = [0.0; 8];
    let mut k = 0;
    while k < 8 {
        axes[k] = k as f64 * std::f64::consts::PI / 8.0;
        k += 1;
    }
    axes
};

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

/// Compute the inclusive pixel bounds of the square centred at `(cx, cy)`
/// with half-extent `reach`, clipped to a `width`×`height` surface.
/// Returns `None` when the square lies entirely off-surface (including
/// empty surfaces), so callers can skip drawing.  Clamping happens on i32
/// values BEFORE any u32 cast: a far-negative centre must not wrap around
/// to a huge unsigned bound.
fn clipped_bounds(
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    reach: f64,
) -> Option<(u32, u32, u32, u32)> {
    let w = width as i32;
    let h = height as i32;
    if w == 0 || h == 0 {
        return None;
    }
    // f64 → i32 casts saturate, so arbitrarily large positions are safe.
    let x0 = ((cx - reach).floor() as i32).clamp(0, w - 1);
    let y0 = ((cy - reach).floor() as i32).clamp(0, h - 1);
    let x1 = ((cx + reach).ceil() as i32).clamp(0, w - 1);
    let y1 = ((cy + reach).ceil() as i32).clamp(0, h - 1);
    // Fully off-canvas: the unclamped extent never intersects the image.
    if cx + reach < 0.0
        || cy + reach < 0.0
        || cx - reach > (w - 1) as f64
        || cy - reach > (h - 1) as f64
    {
        return None;
    }
    if x0 > x1 || y0 > y1 {
        return None;
    }
    Some((x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Union of two optional inclusive bounding boxes `(x0, y0, x1, y1)`.
fn union_bounds(
    a: Option<(u32, u32, u32, u32)>,
    b: Option<(u32, u32, u32, u32)>,
) -> Option<(u32, u32, u32, u32)> {
    match (a, b) {
        (None, other) | (other, None) => other,
        (Some((ax0, ay0, ax1, ay1)), Some((bx0, by0, bx1, by1))) => {
            Some((ax0.min(bx0), ay0.min(by0), ax1.max(bx1), ay1.max(by1)))
        }
    }
}

/// Bounds a glow may light: `None` for degenerate radii (mirroring
/// [`draw_glow`]'s early-out) or fully off-surface centres.
fn glow_bounds(
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    radius: f64,
) -> Option<(u32, u32, u32, u32)> {
    if radius < 1.0 {
        return None;
    }
    clipped_bounds(width, height, cx, cy, radius)
}

/// Radial glow with quadratic falloff: alpha = (1 - d/radius)² × brightness.
fn draw_glow(img: &mut RgbaImage, cx: f64, cy: f64, radius: f64, color: [u8; 3], brightness: f64) {
    let Some((x0, y0, x1, y1)) = glow_bounds(img.width(), img.height(), cx, cy, radius) else {
        return;
    };

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
    let Some((x0, y0, x1, y1)) = clipped_bounds(img.width(), img.height(), cx, cy, length) else {
        return;
    };

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
            let min_angular_dist = SPOKE_AXES
                .iter()
                .map(|&sa| {
                    let diff = (angle_mod - sa).abs();
                    diff.min(std::f64::consts::PI - diff)
                })
                .fold(f64::MAX, f64::min);

            let streak_width_px = (1.5 + 2.0 * (1.0 - d / length)).max(0.5);
            let pixel_perp = min_angular_dist * d;
            if pixel_perp < streak_width_px {
                let t_len = 1.0 - d / length;
                let t_width = 1.0 - pixel_perp / streak_width_px;
                let alpha = (t_len * t_width * brightness * 0.6 * 255.0).clamp(0.0, 255.0) as u8;
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
    let Some((x0, y0, x1, y1)) = clipped_bounds(img.width(), img.height(), cx, cy, outer) else {
        return;
    };

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

/// Render the lens flare for `layer` at `position` on frame `frame_index`.
///
/// Returns a buffer sized to the inclusive canvas-space bounding box
/// `(x0, y0, x1, y1)` of the region the flare elements may light (the union
/// of every element's clipped bounds, clamped to the canvas) together with
/// that box; buffer pixel `(bx, by)` corresponds to canvas pixel
/// `(x0 + bx, y0 + by)`.  When every element is fully off-canvas the bounds
/// are `None` and the buffer is a 1×1 transparent placeholder.  The
/// compositor blends the buffer additively onto the base frame at the box's
/// origin — allocating only the lit region instead of a full canvas.
pub fn render_flare(
    layer: &FlareLayer,
    position: (f64, f64),
    frame_index: usize,
    canvas_width: u32,
    canvas_height: u32,
) -> (RgbaImage, Option<(u32, u32, u32, u32)>) {
    let (ox, oy) = position;

    // Pulsing brightness multiplier
    let brightness =
        (layer.intensity * (1.0 + 0.3 * (frame_index as f64 * layer.pulse_speed).sin())).min(2.0);

    // Blue ghost artifacts sit along the axis from the flare origin through
    // the canvas centre; precompute their (centre, radius, brightness) so
    // both passes below agree exactly.
    let ccx = canvas_width as f64 / 2.0;
    let ccy = canvas_height as f64 / 2.0;
    let axis_x = ccx - ox;
    let axis_y = ccy - oy;
    let ghosts: [(f64, f64, f64, f64); 4] = std::array::from_fn(|i| {
        let phase = i as f64 * 0.5;
        let gb = brightness
            * GHOST_ALPHAS[i]
            * (1.0 + 0.3 * (frame_index as f64 * layer.pulse_speed + phase).sin());
        let gx = ox + axis_x * GHOST_OFFSETS[i];
        let gy = oy + axis_y * GHOST_OFFSETS[i];
        let gr = layer.scale * 80.0 * GHOST_SIZES[i];
        (gx, gy, gr, gb)
    });

    // Element geometry, computed ONCE and fed to both the bounds pass and the
    // draw calls below so the two can never disagree on an element's reach.
    let glow_r = layer.scale * 80.0;
    let burst_len = layer.scale * 200.0;
    let ring_radius = layer.scale * 100.0;
    let ring_thickness = layer.scale * 25.0;
    let halo_r = layer.scale * 140.0;

    // Pass 1: union of every element's canvas-clipped bounds, using the same
    // clipping (and glow degenerate-radius early-out) as the draw calls.
    // Central white glow, starburst, yellow ring (outer = radius+thickness),
    // orange halo, then the four ghosts.
    let mut lit = glow_bounds(canvas_width, canvas_height, ox, oy, glow_r);
    lit = union_bounds(
        lit,
        clipped_bounds(canvas_width, canvas_height, ox, oy, burst_len),
    );
    lit = union_bounds(
        lit,
        clipped_bounds(
            canvas_width,
            canvas_height,
            ox,
            oy,
            ring_radius + ring_thickness,
        ),
    );
    lit = union_bounds(
        lit,
        glow_bounds(canvas_width, canvas_height, ox, oy, halo_r),
    );
    for &(gx, gy, gr, _) in &ghosts {
        lit = union_bounds(lit, glow_bounds(canvas_width, canvas_height, gx, gy, gr));
    }

    let Some((bx0, by0, bx1, by1)) = lit else {
        return (RgbaImage::new(1, 1), None);
    };

    // Pass 2: draw into a buffer covering only the union box, shifting every
    // element centre into buffer-local coordinates.  Per-pixel maths sees the
    // same (x - cx, y - cy) deltas as a full-canvas draw, so the lit pixels
    // are identical — only the allocation shrinks.
    let mut img = RgbaImage::new(bx1 - bx0 + 1, by1 - by0 + 1);
    let (sx, sy) = (bx0 as f64, by0 as f64);

    // 1. Central white glow
    draw_glow(
        &mut img,
        ox - sx,
        oy - sy,
        glow_r,
        [255, 255, 255],
        brightness,
    );

    // 2. Starburst (8 streaks, white)
    draw_starburst(&mut img, ox - sx, oy - sy, burst_len, brightness);

    // 3. Yellow inner ring + soft orange outer halo
    draw_ring(
        &mut img,
        ox - sx,
        oy - sy,
        ring_radius,
        ring_thickness,
        [0xFF, 0xE8, 0x7C], // #FFE87C yellow
        brightness,
    );
    draw_glow(
        &mut img,
        ox - sx,
        oy - sy,
        halo_r,
        [0xFF, 0x7B, 0x00], // #FF7B00 orange
        brightness * 0.4,
    );

    // 4. Blue ghost artifacts
    for &(gx, gy, gr, gb) in &ghosts {
        draw_glow(&mut img, gx - sx, gy - sy, gr, [0x4B, 0x6E, 0xAF], gb); // #4B6EAF blue
    }

    (img, Some((bx0, by0, bx1, by1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reconstruct the full-canvas image implied by a `(buffer, bounds)`
    /// pair: paste the offset buffer at its canvas-space origin onto a
    /// transparent canvas.  Also asserts the buffer is sized exactly to the
    /// reported bounds, which every `Some` result must satisfy.
    fn to_canvas(
        img: &RgbaImage,
        bounds: Option<(u32, u32, u32, u32)>,
        w: u32,
        h: u32,
    ) -> RgbaImage {
        let mut canvas = RgbaImage::new(w, h);
        if let Some((x0, y0, x1, y1)) = bounds {
            assert_eq!(
                img.dimensions(),
                (x1 - x0 + 1, y1 - y0 + 1),
                "buffer must be sized exactly to the reported bounds"
            );
            assert!(
                x1 < w && y1 < h,
                "bounds ({x0}, {y0})-({x1}, {y1}) must lie within the {w}x{h} canvas"
            );
            for (x, y, p) in img.enumerate_pixels() {
                canvas.put_pixel(x + x0, y + y0, *p);
            }
        }
        canvas
    }

    #[test]
    fn render_flare_buffer_matches_reported_bounds() {
        let layer = FlareLayer::new();
        let (img, bounds) = render_flare(&layer, (50.0, 50.0), 0, 100, 80);
        // to_canvas asserts buffer size == bounds size and bounds ⊆ canvas.
        let canvas = to_canvas(&img, bounds, 100, 80);
        assert_eq!(canvas.width(), 100);
        assert_eq!(canvas.height(), 80);
    }

    #[test]
    fn render_flare_center_pixel_is_nonzero_at_high_intensity() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let (img, bounds) = render_flare(&layer, (50.0, 50.0), 0, 100, 100);
        let canvas = to_canvas(&img, bounds, 100, 100);
        let p = canvas.get_pixel(50, 50);
        assert!(p[3] > 0, "center pixel alpha should be > 0; got {}", p[3]);
    }

    /// The central glow is white, so all three colour channels must be lit up
    /// near the flare origin — not just the alpha channel.
    #[test]
    fn render_flare_center_region_has_nonzero_rgb() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let (img, bounds) = render_flare(&layer, (50.0, 50.0), 0, 100, 100);
        let canvas = to_canvas(&img, bounds, 100, 100);
        let p = canvas.get_pixel(50, 50);
        assert!(p[0] > 0, "center pixel R should be > 0; got {}", p[0]);
        assert!(p[1] > 0, "center pixel G should be > 0; got {}", p[1]);
        assert!(p[2] > 0, "center pixel B should be > 0; got {}", p[2]);
    }

    /// The buffer is sized exactly to the reported bounding box, so every lit
    /// pixel is inside the box by construction; the box itself must lie
    /// within the canvas and actually contain lit pixels.
    #[test]
    fn render_flare_bounds_are_in_canvas_and_lit() {
        let mut layer = FlareLayer::new();
        layer.intensity = 1.5;
        layer.scale = 0.2;
        let (img, bounds) = render_flare(&layer, (30.0, 40.0), 2, 120, 90);
        assert!(
            bounds.is_some(),
            "in-canvas flare must report a bounding box"
        );
        // to_canvas asserts buffer size == bounds size and bounds ⊆ canvas.
        let canvas = to_canvas(&img, bounds, 120, 90);
        assert!(
            canvas.pixels().any(|p| p[3] > 0),
            "an in-canvas flare must light at least one pixel"
        );
    }

    /// A flare whose every element is off-canvas must report `None` bounds so
    /// the compositor can skip blending entirely.  Note the offset-1.0 ghost
    /// artifact always lands at the canvas centre regardless of the flare
    /// position, so a tiny scale is needed to make it degenerate (radius < 1)
    /// and get a truly element-free canvas.
    #[test]
    fn render_flare_fully_off_canvas_reports_no_bounds() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        layer.scale = 0.01; // centre ghost radius = 0.01*80*0.4 = 0.32 < 1
        let (img, bounds) = render_flare(&layer, (-10000.0, -10000.0), 0, 100, 100);
        assert_eq!(bounds, None, "fully off-canvas flare must report no bounds");
        assert!(
            img.pixels().all(|p| *p == image::Rgba([0, 0, 0, 0])),
            "no pixel may be lit when bounds are None"
        );
    }

    /// A flare dragged far off-canvas (negative coordinates) must complete
    /// quickly instead of wrapping its negative bounds to ~4.29 billion via
    /// an `as u32` cast and hanging for minutes.
    #[test]
    fn render_flare_far_negative_position_completes_quickly() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let start = std::time::Instant::now();
        let (img, bounds) = render_flare(&layer, (-10000.0, -10000.0), 0, 100, 100);
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "off-canvas flare render took {:?}",
            start.elapsed()
        );
        // Ghost artifacts on the axis through the canvas centre can still be
        // on-canvas; whatever is reported must reconstruct to a 100×100 view.
        let canvas = to_canvas(&img, bounds, 100, 100);
        assert_eq!(canvas.width(), 100);
        assert_eq!(canvas.height(), 100);
    }

    /// Same guarantee for a flare dragged far past the right/bottom edge.
    #[test]
    fn render_flare_far_positive_position_completes_quickly() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let start = std::time::Instant::now();
        let (img, bounds) = render_flare(&layer, (10000.0, 10000.0), 0, 100, 100);
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "off-canvas flare render took {:?}",
            start.elapsed()
        );
        let canvas = to_canvas(&img, bounds, 100, 100);
        assert_eq!(canvas.width(), 100);
        assert_eq!(canvas.height(), 100);
    }

    /// A flare partially off the left edge still draws its on-canvas portion.
    #[test]
    fn render_flare_partially_off_canvas_still_draws() {
        let mut layer = FlareLayer::new();
        layer.intensity = 2.0;
        let (img, bounds) = render_flare(&layer, (-20.0, 50.0), 0, 100, 100);
        let canvas = to_canvas(&img, bounds, 100, 100);
        let p = canvas.get_pixel(0, 50);
        assert!(p[3] > 0, "edge pixel alpha should be > 0; got {}", p[3]);
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
        let (img, bounds) = render_flare(&layer, (30.0, 30.0), 0, 60, 60);
        let canvas = to_canvas(&img, bounds, 60, 60);
        let corner = canvas.get_pixel(0, 0);
        assert!(
            corner[3] < 10,
            "corner pixel alpha should be near-zero for small-scale flare; got {}",
            corner[3]
        );
    }
}
