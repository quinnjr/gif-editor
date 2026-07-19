use gif_editor_lib::compositor::composite_frame;
use gif_editor_lib::layer::{ImageLayer, Keyframe, Layer, TextLayer};
use image::{Rgba, RgbaImage};

fn red_10x10() -> RgbaImage {
    RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]))
}
fn blue_5x5() -> RgbaImage {
    RgbaImage::from_pixel(5, 5, Rgba([0, 0, 255, 255]))
}

#[test]
fn composite_no_layers_returns_base() {
    let base = red_10x10();
    let result = composite_frame(&base, &[], 0);
    assert_eq!(result.dimensions(), (10, 10));
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_image_layer_overlays_at_position() {
    let base = red_10x10();
    let overlay = blue_5x5();
    let mut layer = ImageLayer::new("blue".into(), 5, 5);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (2.0, 3.0);
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(2, 3), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_respects_frame_range() {
    let base = red_10x10();
    let overlay = blue_5x5();
    let mut layer = ImageLayer::new("blue".into(), 5, 5);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (0.0, 0.0);
    layer.frame_range = (2, 5);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255])); // outside range
    let result = composite_frame(&base, &layers, 3);
    assert_eq!(*result.get_pixel(0, 0), Rgba([0, 0, 255, 255])); // inside range
}

#[test]
fn composite_respects_visibility() {
    let base = red_10x10();
    let overlay = blue_5x5();
    let mut layer = ImageLayer::new("blue".into(), 5, 5);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (0.0, 0.0);
    layer.frame_range = (0, 0);
    layer.visible = false;
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_image_with_half_opacity() {
    let base = red_10x10();
    let overlay = blue_5x5();
    let mut layer = ImageLayer::new("blue".into(), 5, 5);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (0.0, 0.0);
    layer.frame_range = (0, 0);
    layer.opacity = 0.5;
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    let pixel = *result.get_pixel(0, 0);
    assert!(pixel[0] > 100 && pixel[0] < 160); // red blended
    assert!(pixel[2] > 100 && pixel[2] < 160); // blue blended
}

#[test]
fn composite_image_with_scale_x() {
    let base = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
    let overlay = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));
    let mut layer = ImageLayer::new("blue".into(), 4, 4);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (0.0, 0.0);
    layer.scale_x = 2.0;
    layer.scale_y = 1.0;
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(7, 2), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(9, 2), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_identity_transform_matches_original() {
    let base = red_10x10();
    let overlay = blue_5x5();
    let mut layer = ImageLayer::new("blue".into(), 5, 5);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (2.0, 3.0);
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(2, 3), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
    assert_eq!(*result.get_pixel(6, 7), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(7, 8), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_image_with_skew_x() {
    let base = RgbaImage::from_pixel(30, 30, Rgba([255, 0, 0, 255]));
    let overlay = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 255, 255]));
    let mut layer = ImageLayer::new("blue".into(), 10, 10);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.position = (5.0, 5.0);
    layer.skew_x = 0.5;
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(5, 5), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_text_layer_adds_visible_pixels() {
    let base_large = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 0, 255]));
    let mut layer = TextLayer::new("Hi".to_string());
    layer.position = (10.0, 10.0);
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Text(layer)];
    let result = composite_frame(&base_large, &layers, 0);
    let has_text = result.pixels().any(|p| *p != Rgba([255, 0, 0, 255]));
    assert!(has_text, "Text layer should modify some pixels");
}

#[test]
fn composite_text_layer_anchors_glyph_box_at_position() {
    // The rendered text buffer carries a stroke pad margin; the compositor
    // must anchor the glyph box (not the pad edge) at the layer position, so
    // moving the layer by (dx, dy) shifts every text pixel by exactly that
    // amount and the pad offset never leaks into placement.
    let base = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 0, 255]));
    let mut layer = TextLayer::new("Hi".to_string());
    layer.position = (60.0, 60.0);
    layer.frame_range = (0, 0);
    let pad = gif_editor_lib::text_renderer::stroke_pad(&layer);
    assert!(pad > 0, "default stroke should produce a nonzero pad");

    let at_60 = composite_frame(&base, &[Layer::Text(layer.clone())], 0);
    layer.position = (60.0 + pad as f64, 60.0);
    let shifted = composite_frame(&base, &[Layer::Text(layer)], 0);

    // The second composite must equal the first translated right by `pad` px.
    let mut matches = true;
    for y in 0..200u32 {
        for x in 0..(200 - pad) {
            if at_60.get_pixel(x, y) != shifted.get_pixel(x + pad, y) {
                matches = false;
            }
        }
    }
    assert!(
        matches,
        "translating the layer must translate all text pixels"
    );

    // With the pad compensated, stroke pixels are allowed to overflow to the
    // left/above the anchor, and glyph content begins at the anchor rather
    // than pad px past it: some non-background pixel must exist within the
    // first pad columns/rows around x=60 that the old anchoring placed later.
    let near_anchor = (55..65)
        .any(|x| (55..75).any(|y| *at_60.get_pixel(x as u32, y as u32) != Rgba([255, 0, 0, 255])));
    assert!(
        near_anchor,
        "text (incl. stroke) should start at the anchor, not pad px past it"
    );
}

// ---------------------------------------------------------------------------
// Text layer with keyframes (interpolated position)
// ---------------------------------------------------------------------------

/// Bounding box (min_x, min_y, max_x, max_y) of pixels differing from the
/// solid red base, or `None` when nothing was modified.
fn modified_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut bbox: Option<(u32, u32, u32, u32)> = None;
    for (x, y, p) in img.enumerate_pixels() {
        if *p != Rgba([255, 0, 0, 255]) {
            bbox = Some(match bbox {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    bbox
}

/// Largest per-pixel channel distance from the solid red base.  White text
/// blended at effective alpha `a` yields a max distance of ~510·a (G and B
/// rise from 0 to 255·a), so this grows monotonically with layer opacity.
fn max_dist_from_red(img: &RgbaImage) -> u32 {
    img.pixels()
        .map(|p| (255 - p[0] as u32) + p[1] as u32 + p[2] as u32)
        .max()
        .unwrap_or(0)
}

#[test]
fn composite_text_layer_with_keyframes() {
    let base = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 0, 255]));
    let mut layer = TextLayer::new("KF".to_string());
    layer.frame_range = (0, 10);
    layer.keyframes = vec![
        Keyframe {
            frame: 0,
            position: (0.0, 0.0),
            opacity: 1.0,
        },
        Keyframe {
            frame: 10,
            position: (100.0, 100.0),
            opacity: 0.5,
        },
    ];
    let layers = vec![Layer::Text(layer)];

    // At frame 0, text should be near (0,0)
    let result0 = composite_frame(&base, &layers, 0);
    // At frame 5, text should be near (50,50) with opacity 0.75
    let result5 = composite_frame(&base, &layers, 5);
    // At frame 10, text should be near (100,100) with opacity 0.5
    let result10 = composite_frame(&base, &layers, 10);

    // The top-left of the modified-pixel bounding box must track the
    // keyframed anchor.  Tolerance is generous to absorb stroke overflow
    // (a few px above/left of the anchor) and glyph bearing/ascent gap
    // (a handful of px right/below it), but far smaller than the 50 px
    // spacing between keyframed positions, so a build that ignores
    // interpolation (rendering every frame at one keyframe's position)
    // fails the frame-5 and/or frame-10 range checks below.
    let (x0, y0, _, _) = modified_bbox(&result0).expect("frame 0 should have text pixels");
    assert!(
        x0 <= 20 && y0 <= 20,
        "frame 0 text should be near (0,0); bbox top-left ({x0}, {y0})"
    );

    let (x5, y5, _, _) = modified_bbox(&result5).expect("frame 5 should have text pixels");
    assert!(
        (30..=70).contains(&x5) && (30..=70).contains(&y5),
        "frame 5 text should be near (50,50); bbox top-left ({x5}, {y5})"
    );

    let (x10, y10, _, _) = modified_bbox(&result10).expect("frame 10 should have text pixels");
    assert!(
        (80..=120).contains(&x10) && (80..=120).contains(&y10),
        "frame 10 text should be near (100,100); bbox top-left ({x10}, {y10})"
    );

    // Opacity must interpolate as well: frame 5 blends at 0.75, frame 10 at
    // 0.5, so frame 5's strongest text pixel sits strictly farther from the
    // red base.  A build that pins opacity to either keyframe's value (or to
    // the layer default) renders both frames at equal strength and fails the
    // strict inequality.
    let d5 = max_dist_from_red(&result5);
    let d10 = max_dist_from_red(&result10);
    assert!(
        d5 > d10,
        "frame 5 (opacity 0.75) should blend more strongly than frame 10 (opacity 0.5): {d5} vs {d10}"
    );
}

// ---------------------------------------------------------------------------
// Image layer with keyframes (interpolated position)
// ---------------------------------------------------------------------------

#[test]
fn composite_image_layer_with_keyframes() {
    let base = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
    let overlay = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));
    let mut layer = ImageLayer::new("blue".into(), 4, 4);
    layer.image_data = Some(std::sync::Arc::new(overlay));
    layer.frame_range = (0, 10);
    layer.keyframes = vec![
        Keyframe {
            frame: 0,
            position: (0.0, 0.0),
            opacity: 1.0,
        },
        Keyframe {
            frame: 10,
            position: (16.0, 16.0),
            opacity: 1.0,
        },
    ];
    let layers = vec![Layer::Image(layer)];

    // At frame 0, overlay is at (0,0)
    let result0 = composite_frame(&base, &layers, 0);
    assert_eq!(*result0.get_pixel(0, 0), Rgba([0, 0, 255, 255]));
    assert_eq!(*result0.get_pixel(16, 16), Rgba([255, 0, 0, 255]));

    // At frame 10, overlay is at (16,16)
    let result10 = composite_frame(&base, &layers, 10);
    assert_eq!(*result10.get_pixel(16, 16), Rgba([0, 0, 255, 255]));
    assert_eq!(*result10.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}

// ---------------------------------------------------------------------------
// Text layer with identity transform (no scale/skew)
// ---------------------------------------------------------------------------

#[test]
fn composite_identity_transform_text_layer() {
    let base = RgbaImage::from_pixel(200, 200, Rgba([255, 0, 0, 255]));
    let mut layer = TextLayer::new("Id".to_string());
    layer.position = (10.0, 10.0);
    layer.scale_x = 1.0;
    layer.scale_y = 1.0;
    layer.skew_x = 0.0;
    layer.skew_y = 0.0;
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Text(layer)];
    let result = composite_frame(&base, &layers, 0);
    // Should have some text pixels drawn
    let has_text = result.pixels().any(|p| *p != Rgba([255, 0, 0, 255]));
    assert!(has_text, "Identity-transform text should still render");
}

// ---------------------------------------------------------------------------
// Animated GIF overlay (multi-frame image layer)
// ---------------------------------------------------------------------------

#[test]
fn composite_animated_gif_overlay_cycles_frames() {
    let base = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
    let frame0 = RgbaImage::from_pixel(4, 4, Rgba([0, 255, 0, 255]));
    let frame1 = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));

    let mut layer = ImageLayer::new("anim".into(), 4, 4);
    layer.frames = std::sync::Arc::new(vec![frame0, frame1]);
    layer.frame_range = (0, 5);
    let layers = vec![Layer::Image(layer)];

    // Frame 0 of the project -> anim frame 0 (green)
    let r0 = composite_frame(&base, &layers, 0);
    assert_eq!(*r0.get_pixel(0, 0), Rgba([0, 255, 0, 255]));

    // Frame 1 of the project -> anim frame 1 (blue)
    let r1 = composite_frame(&base, &layers, 1);
    assert_eq!(*r1.get_pixel(0, 0), Rgba([0, 0, 255, 255]));

    // Frame 2 of the project -> anim frame 0 again (wraps)
    let r2 = composite_frame(&base, &layers, 2);
    assert_eq!(*r2.get_pixel(0, 0), Rgba([0, 255, 0, 255]));
}

#[test]
fn rotation_90_covers_transposed_area() {
    use gif_editor_lib::compositor::composite_frame;
    use gif_editor_lib::layer::{ImageLayer, Layer};
    use image::{Rgba, RgbaImage};

    // 10×20 red source layer (tall rectangle)
    let mut src = RgbaImage::new(10, 20);
    for pixel in src.pixels_mut() {
        *pixel = Rgba([255, 0, 0, 255]);
    }
    let base = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));

    let mut layer = ImageLayer::new("test".to_string(), 10, 20);
    layer.image_data = Some(std::sync::Arc::new(src));
    layer.frame_range = (0, 0);
    layer.rotation = 90.0;
    layer.position = (10.0, 10.0);
    let layers = vec![Layer::Image(layer)];

    let result = composite_frame(&base, &layers, 0);
    // After 90° rotation around origin, the 10×20 rect maps to x=[-20,0], y=[0,10]
    // With position offset (10,10), it becomes x=[-10,10], y=[10,20]
    // Check that pixel at (5, 15) is red (inside rotated footprint)
    let p = result.get_pixel(5, 15);
    assert_eq!(p[0], 255, "Rotated layer pixel should be red");
    assert_eq!(p[3], 255, "Alpha should be full");
}
