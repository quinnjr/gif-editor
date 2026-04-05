use gif_editor_lib::layer::{ImageLayer, Keyframe, Layer, TextLayer, interpolate_keyframes};

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
    let kfs = vec![Keyframe {
        frame: 5,
        position: (10.0, 20.0),
        opacity: 0.5,
    }];
    assert_eq!(interpolate_keyframes(&kfs, 0), Some(((10.0, 20.0), 0.5)));
    assert_eq!(interpolate_keyframes(&kfs, 5), Some(((10.0, 20.0), 0.5)));
    assert_eq!(interpolate_keyframes(&kfs, 10), Some(((10.0, 20.0), 0.5)));
}

#[test]
fn interpolate_two_keyframes_lerps() {
    let kfs = vec![
        Keyframe {
            frame: 0,
            position: (0.0, 0.0),
            opacity: 1.0,
        },
        Keyframe {
            frame: 10,
            position: (100.0, 50.0),
            opacity: 0.0,
        },
    ];
    let result = interpolate_keyframes(&kfs, 5).unwrap();
    assert!((result.0.0 - 50.0).abs() < 0.01);
    assert!((result.0.1 - 25.0).abs() < 0.01);
    assert!((result.1 - 0.5).abs() < 0.01);
}

#[test]
fn interpolate_clamps_before_and_after() {
    let kfs = vec![
        Keyframe {
            frame: 5,
            position: (10.0, 10.0),
            opacity: 0.8,
        },
        Keyframe {
            frame: 15,
            position: (20.0, 20.0),
            opacity: 0.2,
        },
    ];
    assert_eq!(interpolate_keyframes(&kfs, 0), Some(((10.0, 10.0), 0.8)));
    assert_eq!(interpolate_keyframes(&kfs, 20), Some(((20.0, 20.0), 0.2)));
}

// ---------------------------------------------------------------------------
// Layer::keyframes() accessor
// ---------------------------------------------------------------------------

#[test]
fn image_layer_keyframes_accessor() {
    let mut layer = ImageLayer::new("test".to_string(), 10, 10);
    layer.keyframes = vec![
        Keyframe {
            frame: 0,
            position: (1.0, 2.0),
            opacity: 1.0,
        },
        Keyframe {
            frame: 5,
            position: (3.0, 4.0),
            opacity: 0.5,
        },
    ];
    let wrapped = Layer::Image(layer);
    let kfs = wrapped.keyframes();
    assert_eq!(kfs.len(), 2);
    assert_eq!(kfs[0].frame, 0);
    assert_eq!(kfs[1].frame, 5);
}

#[test]
fn text_layer_keyframes_accessor() {
    let mut layer = TextLayer::new("test".to_string());
    layer.keyframes = vec![Keyframe {
        frame: 2,
        position: (10.0, 20.0),
        opacity: 0.8,
    }];
    let wrapped = Layer::Text(layer);
    let kfs = wrapped.keyframes();
    assert_eq!(kfs.len(), 1);
    assert_eq!(kfs[0].frame, 2);
    assert_eq!(kfs[0].position, (10.0, 20.0));
}

#[test]
fn empty_keyframes_accessor() {
    let layer = Layer::Image(ImageLayer::new("no-kf".to_string(), 5, 5));
    assert!(layer.keyframes().is_empty());
}

// ---------------------------------------------------------------------------
// ImageLayer::new defaults
// ---------------------------------------------------------------------------

#[test]
fn image_layer_new_has_empty_frames_and_keyframes() {
    let layer = ImageLayer::new("img".to_string(), 32, 16);
    assert!(layer.frames.is_empty());
    assert!(layer.keyframes.is_empty());
    assert!(layer.image_data.is_none());
    assert!(layer.source_path.is_none());
    assert_eq!(layer.source_width, 32);
    assert_eq!(layer.source_height, 16);
}

// ---------------------------------------------------------------------------
// Layer::id() and Layer::visible()
// ---------------------------------------------------------------------------

#[test]
fn layer_id_delegates_to_inner() {
    let img = ImageLayer::new("a".to_string(), 1, 1);
    let expected_id = img.id;
    let layer = Layer::Image(img);
    assert_eq!(layer.id(), expected_id);
}

#[test]
fn layer_visible_delegates_to_inner() {
    let mut txt = TextLayer::new("v".to_string());
    txt.visible = false;
    let layer = Layer::Text(txt);
    assert!(!layer.visible());
}

#[test]
fn layer_frame_range_delegates_to_inner() {
    let mut img = ImageLayer::new("r".to_string(), 1, 1);
    img.frame_range = (3, 7);
    let layer = Layer::Image(img);
    assert_eq!(layer.frame_range(), (3, 7));
}
