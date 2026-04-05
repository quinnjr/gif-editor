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
    layer.image_data = Some(overlay);
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
    layer.image_data = Some(overlay);
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
    layer.image_data = Some(overlay);
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
    layer.image_data = Some(overlay);
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
    layer.image_data = Some(overlay);
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
    layer.image_data = Some(overlay);
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
    layer.image_data = Some(overlay);
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

// ---------------------------------------------------------------------------
// Text layer with keyframes (interpolated position)
// ---------------------------------------------------------------------------

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

    // All three composites should produce valid images
    assert_eq!(result0.dimensions(), (200, 200));
    assert_eq!(result5.dimensions(), (200, 200));
    assert_eq!(result10.dimensions(), (200, 200));

    // The text should render somewhere different in each frame
    let modified_0 = result0
        .pixels()
        .filter(|p| **p != Rgba([255, 0, 0, 255]))
        .count();
    let modified_5 = result5
        .pixels()
        .filter(|p| **p != Rgba([255, 0, 0, 255]))
        .count();
    assert!(modified_0 > 0, "Frame 0 should have text pixels");
    assert!(modified_5 > 0, "Frame 5 should have text pixels");
}

// ---------------------------------------------------------------------------
// Image layer with keyframes (interpolated position)
// ---------------------------------------------------------------------------

#[test]
fn composite_image_layer_with_keyframes() {
    let base = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
    let overlay = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));
    let mut layer = ImageLayer::new("blue".into(), 4, 4);
    layer.image_data = Some(overlay);
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
    layer.frames = vec![frame0, frame1];
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
