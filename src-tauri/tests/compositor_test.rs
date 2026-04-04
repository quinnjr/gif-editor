use gif_editor_lib::compositor::composite_frame;
use gif_editor_lib::layer::{ImageLayer, Layer, TextLayer};
use image::{Rgba, RgbaImage};

fn red_10x10() -> RgbaImage { RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255])) }
fn blue_5x5() -> RgbaImage { RgbaImage::from_pixel(5, 5, Rgba([0, 0, 255, 255])) }

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
