use std::time::Instant;

use gif_editor_lib::compositor::composite_frame;
use gif_editor_lib::flare_renderer::render_flare;
use gif_editor_lib::layer::{FlareLayer, Layer};
use image::{Rgba, RgbaImage};

fn gray_base(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([40, 40, 40, 255]))
}

fn flare_at(x: f64, y: f64) -> FlareLayer {
    let mut layer = FlareLayer::new();
    layer.position = (x, y);
    layer
}

/// Sum of RGB channels at a pixel — a simple brightness proxy.
fn brightness_at(img: &RgbaImage, x: u32, y: u32) -> u32 {
    let p = img.get_pixel(x, y);
    p[0] as u32 + p[1] as u32 + p[2] as u32
}

#[test]
fn composite_flare_brightens_additively() {
    let base = gray_base(200, 200);
    let layers = vec![Layer::Flare(flare_at(100.0, 100.0))];
    let result = composite_frame(&base, &layers, 0);

    // Additive compositing never darkens any pixel...
    for (x, y, p) in result.enumerate_pixels() {
        let b = base.get_pixel(x, y);
        assert!(
            p[0] >= b[0] && p[1] >= b[1] && p[2] >= b[2],
            "pixel ({x},{y}) darkened: {p:?} < {b:?}"
        );
    }
    // ...and the flare center must be strictly brighter than the base.
    assert!(
        brightness_at(&result, 100, 100) > brightness_at(&base, 100, 100),
        "flare center should brighten the base"
    );
}

#[test]
fn composite_flare_far_off_canvas_completes_quickly() {
    // Regression for the off-canvas integer-wrap hang: far negative and far
    // positive positions must finish immediately instead of looping over
    // billions of wrapped pixel coordinates. Note the base is NOT necessarily
    // unchanged — halo/ghost elements sit on the line through the optical
    // center, so an off-frame light source can still cast on-frame light
    // (matching the client preview).
    let base = gray_base(120, 90);
    for pos in [(-10_000.0, -10_000.0), (10_000.0, 10_000.0)] {
        let layers = vec![Layer::Flare(flare_at(pos.0, pos.1))];
        let start = Instant::now();
        let result = composite_frame(&base, &layers, 0);
        assert!(
            start.elapsed().as_secs() < 5,
            "off-canvas flare at {pos:?} took too long"
        );
        assert_eq!(result.dimensions(), base.dimensions());
        // Additivity must still hold for whatever was lit.
        for (x, y, p) in result.enumerate_pixels() {
            let b = base.get_pixel(x, y);
            assert!(p[0] >= b[0] && p[1] >= b[1] && p[2] >= b[2]);
        }
    }
}

#[test]
fn composite_flare_higher_intensity_is_brighter() {
    let base = gray_base(200, 200);

    let mut dim = flare_at(100.0, 100.0);
    dim.intensity = 0.3;
    let mut bright = flare_at(100.0, 100.0);
    bright.intensity = 1.8;

    let dim_result = composite_frame(&base, &[Layer::Flare(dim)], 0);
    let bright_result = composite_frame(&base, &[Layer::Flare(bright)], 0);

    // Compare slightly off-center so neither sample is clamped at 255.
    let (sx, sy) = (140, 100);
    assert!(
        brightness_at(&bright_result, sx, sy) > brightness_at(&dim_result, sx, sy),
        "intensity 1.8 should render brighter than 0.3 at ({sx},{sy})"
    );
}

#[test]
fn composite_flare_larger_scale_lights_farther_pixels() {
    let base = gray_base(400, 400);

    let mut small = flare_at(200.0, 200.0);
    small.scale = 0.5;
    let mut large = flare_at(200.0, 200.0);
    large.scale = 1.9;

    let small_result = composite_frame(&base, &[Layer::Flare(small)], 0);
    let large_result = composite_frame(&base, &[Layer::Flare(large)], 0);

    // Find the farthest lit pixel from the center along the +x axis.
    let farthest_lit = |img: &RgbaImage| -> u32 {
        (200..400u32)
            .rev()
            .find(|&x| brightness_at(img, x, 200) > brightness_at(&base, x, 200))
            .unwrap_or(200)
    };
    assert!(
        farthest_lit(&large_result) > farthest_lit(&small_result),
        "scale 1.9 should light pixels farther from center than scale 0.5"
    );
}

/// Reconstruct the full-canvas image implied by a `render_flare`
/// `(buffer, bounds)` pair: paste the buffer at its canvas-space origin onto
/// a transparent canvas.
fn paste(img: &RgbaImage, bounds: Option<(u32, u32, u32, u32)>, w: u32, h: u32) -> RgbaImage {
    let mut canvas = RgbaImage::new(w, h);
    if let Some((x0, y0, _, _)) = bounds {
        for (x, y, p) in img.enumerate_pixels() {
            canvas.put_pixel(x + x0, y + y0, *p);
        }
    }
    canvas
}

/// Oracle test for the bounds pass: the bounding box render_flare reports
/// must never clip pixels the draw pass would light.  Ground truth is the
/// same flare rendered on a canvas large enough that nothing is clipped;
/// on a small canvas whose edges cut through the flare, every pixel must
/// match the ground truth exactly.
///
/// Both renders place the flare at the exact centre of their canvas, so the
/// ghost-artifact axis (origin → canvas centre) is zero in both and the lit
/// pattern is identical up to translation — making a pixel-exact comparison
/// valid even though the canvases differ.
#[test]
fn render_flare_bounds_do_not_clip_lit_pixels() {
    for scale in [0.5, 1.0] {
        let mut layer = FlareLayer::new();
        layer.intensity = 1.5;
        layer.scale = scale;

        // Ground truth: 500×500 canvas, flare at its centre (250, 250).  The
        // farthest-reaching element (starburst) extends scale*200 <= 200 px,
        // so no element touches an edge and nothing is clipped.
        let (big_img, big_bounds) = render_flare(&layer, (250.0, 250.0), 0, 500, 500);
        let big = paste(&big_img, big_bounds, 500, 500);

        // Small canvas: 120×120, flare at its centre (60, 60).  Every edge
        // cuts through the flare (reach >= 100 px > 60 px to each edge).
        let (small_img, small_bounds) = render_flare(&layer, (60.0, 60.0), 0, 120, 120);
        let small = paste(&small_img, small_bounds, 120, 120);

        // The cut must actually be exercised: the edge midpoint lies on a
        // horizontal starburst spoke and must be lit.
        assert!(
            small.get_pixel(0, 60)[3] > 0,
            "scale {scale}: small-canvas edge should cut through lit flare"
        );

        // Every small-canvas pixel must equal the unclipped ground truth at
        // the translated position (small (x, y) ↔ big (x + 190, y + 190)).
        for (x, y, p) in small.enumerate_pixels() {
            let truth = big.get_pixel(x + 190, y + 190);
            assert_eq!(
                p, truth,
                "scale {scale}: pixel ({x}, {y}) diverges from unclipped render"
            );
        }
    }
}

#[test]
fn composite_flare_pulse_varies_brightness_across_frames() {
    let base = gray_base(200, 200);
    let mut layer = flare_at(100.0, 100.0);
    layer.pulse_speed = std::f64::consts::FRAC_PI_2; // sin peaks at frame 1, troughs at frame 3
    layer.intensity = 0.8;
    layer.frame_range = (0, 10);
    let layers = vec![Layer::Flare(layer)];

    let peak = composite_frame(&base, &layers, 1);
    let trough = composite_frame(&base, &layers, 3);

    let (sx, sy) = (140, 100);
    assert!(
        brightness_at(&peak, sx, sy) > brightness_at(&trough, sx, sy),
        "pulse peak frame should be brighter than trough frame"
    );
}
