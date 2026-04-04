use gif_editor_lib::layer::{ImageLayer, Layer, Stroke, TextLayer};
use uuid::Uuid;

#[test]
fn image_layer_default_values() {
    let layer = ImageLayer::new("test.png".to_string(), 100, 50);
    assert_eq!(layer.name, "test.png");
    assert_eq!(layer.scale_x, 1.0);
    assert_eq!(layer.scale_y, 1.0);
    assert_eq!(layer.skew_x, 0.0);
    assert_eq!(layer.skew_y, 0.0);
    assert_eq!(layer.opacity, 1.0);
    assert_eq!(layer.frame_range, (0, 0));
    assert!(layer.visible);
}

#[test]
fn text_layer_default_values() {
    let layer = TextLayer::new("Hello".to_string());
    assert_eq!(layer.text, "Hello");
    assert_eq!(layer.font_family, "Impact");
    assert_eq!(layer.font_size, 48.0);
    assert_eq!(layer.color, [255, 255, 255, 255]);
    assert_eq!(layer.scale_x, 1.0);
    assert_eq!(layer.scale_y, 1.0);
    assert_eq!(layer.skew_x, 0.0);
    assert_eq!(layer.skew_y, 0.0);
    assert_eq!(layer.opacity, 1.0);
    assert!(layer.visible);
}

#[test]
fn layer_serializes_to_json() {
    let layer = Layer::Text(TextLayer::new("Meme".to_string()));
    let json = serde_json::to_string(&layer).unwrap();
    assert!(json.contains("\"text\":\"Meme\""));
}
