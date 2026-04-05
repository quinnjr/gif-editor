use gif_editor_lib::layer::{ImageLayer, Keyframe, Layer, Stroke, TextLayer, interpolate_keyframes};
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

#[test]
fn interpolate_empty_keyframes_returns_none() {
    assert_eq!(interpolate_keyframes(&[], 0), None);
}

#[test]
fn interpolate_single_keyframe_returns_its_values() {
    let kfs = vec![Keyframe { frame: 5, position: (10.0, 20.0), opacity: 0.5 }];
    assert_eq!(interpolate_keyframes(&kfs, 0), Some(((10.0, 20.0), 0.5)));
    assert_eq!(interpolate_keyframes(&kfs, 5), Some(((10.0, 20.0), 0.5)));
    assert_eq!(interpolate_keyframes(&kfs, 10), Some(((10.0, 20.0), 0.5)));
}

#[test]
fn interpolate_two_keyframes_lerps() {
    let kfs = vec![
        Keyframe { frame: 0, position: (0.0, 0.0), opacity: 1.0 },
        Keyframe { frame: 10, position: (100.0, 50.0), opacity: 0.0 },
    ];
    let result = interpolate_keyframes(&kfs, 5).unwrap();
    assert!((result.0 .0 - 50.0).abs() < 0.01);
    assert!((result.0 .1 - 25.0).abs() < 0.01);
    assert!((result.1 - 0.5).abs() < 0.01);
}

#[test]
fn interpolate_clamps_before_and_after() {
    let kfs = vec![
        Keyframe { frame: 5, position: (10.0, 10.0), opacity: 0.8 },
        Keyframe { frame: 15, position: (20.0, 20.0), opacity: 0.2 },
    ];
    assert_eq!(interpolate_keyframes(&kfs, 0), Some(((10.0, 10.0), 0.8)));
    assert_eq!(interpolate_keyframes(&kfs, 20), Some(((20.0, 20.0), 0.2)));
}
