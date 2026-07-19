use gif_editor_lib::layer::{ImageLayer, Keyframe, Layer, Stroke, TextLayer};
use gif_editor_lib::project::{LayerInfo, LayerUpdate, Project};
use std::path::Path;
use uuid::Uuid;

fn open_test_gif() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test.gif");
    let (project, _meta) = Project::open(&path).unwrap();
    project
}

fn open_test_png() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test.png");
    let (project, _meta) = Project::open(&path).unwrap();
    project
}

fn png_fixture_path() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test.png")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn logical_to_source_identity_when_no_exclusions() {
    let project = open_test_gif();
    let count = project.visible_frame_count();
    assert!(count > 0);
    for i in 0..count {
        assert_eq!(project.logical_to_source(i), Some(i));
    }
    assert_eq!(project.logical_to_source(count), None);
}

#[test]
fn source_to_logical_identity_when_no_exclusions() {
    let project = open_test_gif();
    let count = project.source.frame_count();
    for i in 0..count {
        assert_eq!(project.source_to_logical(i), Some(i));
    }
}

#[test]
fn logical_to_source_skips_excluded() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3, "test GIF needs at least 3 frames");
    project.excluded_frames.insert(1);
    assert_eq!(project.logical_to_source(0), Some(0));
    assert_eq!(project.logical_to_source(1), Some(2));
    assert_eq!(project.visible_frame_count(), total - 1);
}

#[test]
fn source_to_logical_returns_none_for_excluded() {
    let mut project = open_test_gif();
    project.excluded_frames.insert(1);
    assert_eq!(project.source_to_logical(0), Some(0));
    assert_eq!(project.source_to_logical(1), None);
    assert_eq!(project.source_to_logical(2), Some(1));
}

#[test]
fn visible_delays_excludes_hidden_frames() {
    let mut project = open_test_gif();
    let all_delays = project.source.delays().to_vec();
    let total = all_delays.len();
    assert!(total >= 3);
    project.excluded_frames.insert(1);
    let visible = project.visible_delays();
    assert_eq!(visible.len(), total - 1);
    assert_eq!(visible[0], all_delays[0]);
    assert_eq!(visible[1], all_delays[2]);
}

#[test]
fn visible_metadata_reflects_exclusions() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    project.excluded_frames.insert(0);
    let meta = project.visible_metadata();
    assert_eq!(meta.frame_count, total - 1);
    assert_eq!(meta.delays.len(), total - 1);
}

#[test]
fn delete_frames_excludes_and_adjusts_metadata() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);
    let meta = project.delete_frames(&[1]).unwrap();
    assert_eq!(meta.frame_count, total - 1);
    assert_eq!(project.excluded_frames.len(), 1);
    assert!(project.excluded_frames.contains(&1));
}

#[test]
fn delete_all_frames_rejected() {
    let mut project = open_test_gif();
    let total = project.visible_frame_count();
    let all: Vec<usize> = (0..total).collect();
    let result = project.delete_frames(&all);
    assert!(result.is_err());
}

#[test]
fn delete_frames_with_duplicate_indices_not_spuriously_rejected() {
    let mut project = open_test_gif();
    let total = project.visible_frame_count();
    assert!(total >= 3);
    // Duplicated logical index 0: naive counting would see 3 deletions of a
    // 3-frame GIF and reject, but only 2 distinct frames are being deleted.
    let meta = project.delete_frames(&[0, 0, 1]).unwrap();
    assert_eq!(meta.frame_count, total - 2);
    assert_eq!(project.excluded_frames.len(), 2);
}

#[test]
fn restore_frames_brings_back_excluded() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    project.delete_frames(&[0, 1]).unwrap();
    assert_eq!(project.visible_frame_count(), total - 2);
    let meta = project.restore_frames(&[0]).unwrap();
    assert_eq!(meta.frame_count, total - 1);
    assert!(!project.excluded_frames.contains(&0));
    assert!(project.excluded_frames.contains(&1));
}

#[test]
fn delete_adjusts_layer_frame_ranges() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);
    let mut layer = TextLayer::new("test".to_string());
    layer.frame_range = (0, total - 1);
    project.layers.push(Layer::Text(layer));
    project.delete_frames(&[0]).unwrap();
    let range = project.layers[0].frame_range();
    assert_eq!(range, (0, total - 2));
}

#[test]
fn restore_adjusts_layer_frame_ranges() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);
    let mut layer = TextLayer::new("test".to_string());
    layer.frame_range = (0, total - 1);
    project.layers.push(Layer::Text(layer));
    project.delete_frames(&[0]).unwrap();
    project.restore_frames(&[0]).unwrap();
    let range = project.layers[0].frame_range();
    assert_eq!(range, (0, total - 1));
}

// ---------------------------------------------------------------------------
// From<&Layer> for LayerInfo
// ---------------------------------------------------------------------------

#[test]
fn layer_info_from_image_layer() {
    let mut layer = ImageLayer::new("overlay.png".to_string(), 100, 50);
    layer.position = (10.0, 20.0);
    layer.scale_x = 2.0;
    layer.scale_y = 0.5;
    layer.skew_x = 0.1;
    layer.skew_y = 0.2;
    layer.opacity = 0.75;
    layer.frame_range = (1, 5);
    layer.visible = false;
    layer.source_path = Some("/tmp/overlay.png".to_string());
    layer.keyframes = vec![Keyframe {
        frame: 0,
        position: (0.0, 0.0),
        opacity: 1.0,
    }];

    let info = LayerInfo::from(&Layer::Image(layer.clone()));

    assert_eq!(info.id, layer.id);
    assert_eq!(info.name, "overlay.png");
    assert_eq!(info.layer_type, "image");
    assert_eq!(info.position, (10.0, 20.0));
    assert_eq!(info.scale_x, 2.0);
    assert_eq!(info.scale_y, 0.5);
    assert_eq!(info.skew_x, 0.1);
    assert_eq!(info.skew_y, 0.2);
    assert_eq!(info.opacity, 0.75);
    assert_eq!(info.frame_range, (1, 5));
    assert!(!info.visible);
    assert_eq!(info.source_width, Some(100));
    assert_eq!(info.source_height, Some(50));
    assert_eq!(info.source_path, Some("/tmp/overlay.png".to_string()));
    assert_eq!(info.keyframes.len(), 1);
    // Text-specific fields are None for image layers
    assert!(info.text.is_none());
    assert!(info.font_family.is_none());
    assert!(info.font_size.is_none());
    assert!(info.color.is_none());
    assert!(info.stroke.is_none());
}

#[test]
fn layer_info_from_text_layer() {
    let mut layer = TextLayer::new("Hello World".to_string());
    layer.position = (5.0, 15.0);
    layer.scale_x = 1.5;
    layer.scale_y = 1.5;
    layer.skew_x = 0.3;
    layer.skew_y = 0.4;
    layer.opacity = 0.9;
    layer.frame_range = (2, 8);
    layer.visible = true;
    layer.font_family = "Arial".to_string();
    layer.font_size = 64.0;
    layer.color = [255, 0, 0, 255];
    layer.stroke = Some(Stroke {
        color: [0, 0, 0, 255],
        width: 3.0,
    });
    layer.keyframes = vec![
        Keyframe {
            frame: 2,
            position: (5.0, 15.0),
            opacity: 0.9,
        },
        Keyframe {
            frame: 8,
            position: (50.0, 50.0),
            opacity: 0.1,
        },
    ];

    let info = LayerInfo::from(&Layer::Text(layer.clone()));

    assert_eq!(info.id, layer.id);
    assert_eq!(info.name, "Text: Hello World");
    assert_eq!(info.layer_type, "text");
    assert_eq!(info.position, (5.0, 15.0));
    assert_eq!(info.scale_x, 1.5);
    assert_eq!(info.scale_y, 1.5);
    assert_eq!(info.skew_x, 0.3);
    assert_eq!(info.skew_y, 0.4);
    assert_eq!(info.opacity, 0.9);
    assert_eq!(info.frame_range, (2, 8));
    assert!(info.visible);
    assert_eq!(info.text, Some("Hello World".to_string()));
    assert_eq!(info.font_family, Some("Arial".to_string()));
    assert_eq!(info.font_size, Some(64.0));
    assert_eq!(info.color, Some([255, 0, 0, 255]));
    assert!(info.stroke.is_some());
    assert_eq!(info.keyframes.len(), 2);
    // Image-specific fields are None for text layers
    assert!(info.source_width.is_none());
    assert!(info.source_height.is_none());
    assert!(info.source_path.is_none());
}

// ---------------------------------------------------------------------------
// add_image_layer
// ---------------------------------------------------------------------------

#[test]
fn add_image_layer_with_static_png() {
    let mut project = open_test_gif();
    let frame_count = project.visible_frame_count();
    let (info, meta_change) = project.add_image_layer(&png_fixture_path()).unwrap();

    assert_eq!(info.layer_type, "image");
    assert_eq!(info.source_width, Some(2));
    assert_eq!(info.source_height, Some(2));
    assert_eq!(info.frame_range, (0, frame_count - 1));
    assert!(info.source_path.is_some());
    // Static PNG added to a GIF project should not change metadata
    assert!(meta_change.is_none());
    assert_eq!(project.layers.len(), 1);
}

#[test]
fn add_image_layer_with_gif_overlay_expands_static_image_timeline() {
    let mut project = open_test_png();
    assert_eq!(project.visible_frame_count(), 1);

    let gif_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test.gif")
        .to_string_lossy()
        .into_owned();

    let (info, meta_change) = project.add_image_layer(&gif_path).unwrap();

    assert_eq!(info.layer_type, "image");
    // GIF overlay has 3 frames, so the static image project should expand
    assert!(project.visible_frame_count() >= 3);
    // Metadata should change because the timeline expanded
    assert!(meta_change.is_some());
    let meta = meta_change.unwrap();
    assert!(meta.frame_count >= 3);
}

// ---------------------------------------------------------------------------
// add_text_layer
// ---------------------------------------------------------------------------

#[test]
fn add_text_layer_defaults() {
    let mut project = open_test_gif();
    let frame_count = project.visible_frame_count();
    let info = project.add_text_layer("Meme Text".to_string(), None, None, None, None);

    assert_eq!(info.layer_type, "text");
    assert_eq!(info.text, Some("Meme Text".to_string()));
    assert_eq!(info.font_family, Some("Impact".to_string()));
    assert_eq!(info.font_size, Some(48.0));
    assert_eq!(info.color, Some([255, 255, 255, 255]));
    assert_eq!(info.frame_range, (0, frame_count - 1));
    assert!(info.visible);
    assert_eq!(project.layers.len(), 1);
}

#[test]
fn add_text_layer_with_custom_options() {
    let mut project = open_test_gif();
    let stroke = Stroke {
        color: [255, 0, 0, 255],
        width: 5.0,
    };
    let info = project.add_text_layer(
        "Custom".to_string(),
        Some("Courier".to_string()),
        Some(72.0),
        Some([0, 255, 0, 255]),
        Some(stroke),
    );

    assert_eq!(info.font_family, Some("Courier".to_string()));
    assert_eq!(info.font_size, Some(72.0));
    assert_eq!(info.color, Some([0, 255, 0, 255]));
    assert!(info.stroke.is_some());
}

// ---------------------------------------------------------------------------
// update_layer — Image variant
// ---------------------------------------------------------------------------

#[test]
fn update_image_layer_all_fields() {
    let mut project = open_test_gif();
    project.add_image_layer(&png_fixture_path()).unwrap();
    let layer_id = project.layers[0].id();

    let update = LayerUpdate {
        name: Some("renamed".to_string()),
        position: Some((42.0, 84.0)),
        scale_x: Some(3.0),
        scale_y: Some(2.0),
        skew_x: Some(0.5),
        skew_y: Some(0.6),
        rotation: Some(45.0),
        opacity: Some(0.3),
        frame_range: Some((1, 2)),
        visible: Some(false),
        text: Some("ignored for image".to_string()),
        font_family: Some("ignored".to_string()),
        font_size: Some(99.0),
        color: Some([1, 2, 3, 4]),
        stroke: Some(Some(Stroke {
            color: [5, 6, 7, 8],
            width: 10.0,
        })),
        text_align: None,
        max_width: None,
        keyframes: Some(vec![Keyframe {
            frame: 1,
            position: (10.0, 20.0),
            opacity: 0.5,
        }]),
        intensity: None,
        scale: None,
        pulse_speed: None,
    };

    let info = project.update_layer(layer_id, update).unwrap();
    assert_eq!(info.name, "renamed");
    assert_eq!(info.position, (42.0, 84.0));
    assert_eq!(info.scale_x, 3.0);
    assert_eq!(info.scale_y, 2.0);
    assert_eq!(info.skew_x, 0.5);
    assert_eq!(info.skew_y, 0.6);
    assert_eq!(info.rotation, 45.0);
    assert_eq!(info.opacity, 0.3);
    assert_eq!(info.frame_range, (1, 2));
    assert!(!info.visible);
    assert_eq!(info.keyframes.len(), 1);
    // Text fields should remain None for image layers
    assert!(info.text.is_none());
}

// ---------------------------------------------------------------------------
// update_layer — Text variant
// ---------------------------------------------------------------------------

#[test]
fn update_text_layer_all_fields() {
    let mut project = open_test_gif();
    project.add_text_layer("initial".to_string(), None, None, None, None);
    let layer_id = project.layers[0].id();

    let new_stroke = Stroke {
        color: [10, 20, 30, 255],
        width: 4.0,
    };
    let update = LayerUpdate {
        name: Some("updated-name".to_string()),
        position: Some((100.0, 200.0)),
        scale_x: Some(1.5),
        scale_y: Some(0.8),
        skew_x: Some(0.2),
        skew_y: Some(0.3),
        rotation: Some(90.0),
        opacity: Some(0.6),
        frame_range: Some((0, 1)),
        visible: Some(false),
        text: Some("new text".to_string()),
        font_family: Some("Helvetica".to_string()),
        font_size: Some(36.0),
        color: Some([128, 64, 32, 200]),
        stroke: Some(Some(new_stroke)),
        text_align: Some("left".to_string()),
        max_width: Some(Some(200.0)),
        keyframes: Some(vec![
            Keyframe {
                frame: 0,
                position: (0.0, 0.0),
                opacity: 1.0,
            },
            Keyframe {
                frame: 1,
                position: (50.0, 50.0),
                opacity: 0.0,
            },
        ]),
        intensity: None,
        scale: None,
        pulse_speed: None,
    };

    let info = project.update_layer(layer_id, update).unwrap();
    assert_eq!(info.name, "updated-name");
    assert_eq!(info.position, (100.0, 200.0));
    assert_eq!(info.scale_x, 1.5);
    assert_eq!(info.scale_y, 0.8);
    assert_eq!(info.skew_x, 0.2);
    assert_eq!(info.skew_y, 0.3);
    assert_eq!(info.rotation, 90.0);
    assert_eq!(info.opacity, 0.6);
    assert_eq!(info.frame_range, (0, 1));
    assert!(!info.visible);
    assert_eq!(info.text, Some("new text".to_string()));
    assert_eq!(info.font_family, Some("Helvetica".to_string()));
    assert_eq!(info.font_size, Some(36.0));
    assert_eq!(info.color, Some([128, 64, 32, 200]));
    assert!(info.stroke.is_some());
    assert_eq!(info.keyframes.len(), 2);
}

// ---------------------------------------------------------------------------
// update_layer — clearing nullable fields (double-Option pattern)
// ---------------------------------------------------------------------------

#[test]
fn update_text_layer_explicit_null_clears_max_width_and_stroke() {
    let mut project = open_test_gif();
    project.add_text_layer("clearable".to_string(), None, None, None, None);
    let layer_id = project.layers[0].id();

    // Establish non-default values first.
    let set = LayerUpdate {
        max_width: Some(Some(150.0)),
        stroke: Some(Some(Stroke {
            color: [1, 2, 3, 255],
            width: 2.0,
        })),
        ..Default::default()
    };
    let info = project.update_layer(layer_id, set).unwrap();
    assert_eq!(info.max_width, Some(150.0));
    assert!(info.stroke.is_some());

    // The frontend sends explicit nulls to clear both fields.
    let clear: LayerUpdate =
        serde_json::from_str(r#"{"max_width": null, "stroke": null}"#).unwrap();
    assert_eq!(clear.max_width, Some(None));
    assert!(matches!(clear.stroke, Some(None)));

    let info = project.update_layer(layer_id, clear).unwrap();
    assert_eq!(info.max_width, None);
    assert!(info.stroke.is_none());
}

#[test]
fn update_text_layer_absent_fields_leave_max_width_and_stroke_unchanged() {
    let mut project = open_test_gif();
    project.add_text_layer("keep".to_string(), None, None, None, None);
    let layer_id = project.layers[0].id();

    let set = LayerUpdate {
        max_width: Some(Some(99.0)),
        stroke: Some(Some(Stroke {
            color: [9, 8, 7, 255],
            width: 3.0,
        })),
        ..Default::default()
    };
    project.update_layer(layer_id, set).unwrap();

    // A payload that omits both fields must not touch them.
    let unrelated: LayerUpdate = serde_json::from_str(r#"{"opacity": 0.5}"#).unwrap();
    assert_eq!(unrelated.max_width, None);
    assert!(unrelated.stroke.is_none());

    let info = project.update_layer(layer_id, unrelated).unwrap();
    assert_eq!(info.opacity, 0.5);
    assert_eq!(info.max_width, Some(99.0));
    let stroke = info.stroke.expect("stroke should be unchanged");
    assert_eq!(stroke.color, [9, 8, 7, 255]);
    assert_eq!(stroke.width, 3.0);
}

#[test]
fn update_layer_not_found() {
    let mut project = open_test_gif();
    let bogus_id = Uuid::new_v4();
    let update = LayerUpdate {
        name: Some("x".to_string()),
        position: None,
        scale_x: None,
        scale_y: None,
        skew_x: None,
        skew_y: None,
        rotation: None,
        opacity: None,
        frame_range: None,
        visible: None,
        text: None,
        font_family: None,
        font_size: None,
        color: None,
        stroke: None,
        text_align: None,
        max_width: None,
        keyframes: None,
        intensity: None,
        scale: None,
        pulse_speed: None,
    };
    assert!(project.update_layer(bogus_id, update).is_err());
}

// ---------------------------------------------------------------------------
// remove_layer
// ---------------------------------------------------------------------------

#[test]
fn remove_layer_success() {
    let mut project = open_test_gif();
    project.add_text_layer("to remove".to_string(), None, None, None, None);
    assert_eq!(project.layers.len(), 1);
    let id = project.layers[0].id();
    project.remove_layer(id).unwrap();
    assert_eq!(project.layers.len(), 0);
}

#[test]
fn remove_layer_not_found() {
    let mut project = open_test_gif();
    assert!(project.remove_layer(Uuid::new_v4()).is_err());
}

// ---------------------------------------------------------------------------
// reorder_layers
// ---------------------------------------------------------------------------

#[test]
fn reorder_layers_reverses_order() {
    let mut project = open_test_gif();
    project.add_text_layer("first".to_string(), None, None, None, None);
    project.add_text_layer("second".to_string(), None, None, None, None);
    let id0 = project.layers[0].id();
    let id1 = project.layers[1].id();

    project.reorder_layers(vec![id1, id0]).unwrap();

    assert_eq!(project.layers[0].id(), id1);
    assert_eq!(project.layers[1].id(), id0);
}

#[test]
fn reorder_layers_invalid_id_errors() {
    let mut project = open_test_gif();
    project.add_text_layer("a".to_string(), None, None, None, None);
    assert!(project.reorder_layers(vec![Uuid::new_v4()]).is_err());
}

// ---------------------------------------------------------------------------
// render_composite
// ---------------------------------------------------------------------------

#[test]
fn render_composite_returns_valid_png_path() {
    let mut project = open_test_gif();
    let path = project.render_composite(0).unwrap();
    assert!(std::path::Path::new(&path).exists());
    // Verify it's a valid PNG by loading it
    let img = image::open(&path).unwrap();
    assert_eq!(img.width(), 10);
    assert_eq!(img.height(), 10);
}

#[test]
fn render_composite_with_layers() {
    let mut project = open_test_gif();
    project.add_text_layer("overlay".to_string(), None, None, None, None);
    project.add_image_layer(&png_fixture_path()).unwrap();
    let path = project.render_composite(0).unwrap();
    assert!(std::path::Path::new(&path).exists());
}

#[test]
fn render_composite_out_of_bounds() {
    let mut project = open_test_gif();
    let count = project.visible_frame_count();
    assert!(project.render_composite(count).is_err());
}

// ---------------------------------------------------------------------------
// get_layers
// ---------------------------------------------------------------------------

#[test]
fn get_layers_returns_all_layers() {
    let mut project = open_test_gif();
    project.add_text_layer("a".to_string(), None, None, None, None);
    project.add_text_layer("b".to_string(), None, None, None, None);
    let infos = project.get_layers();
    assert_eq!(infos.len(), 2);
}

// ---------------------------------------------------------------------------
// delete_frames with layer keyframes
// ---------------------------------------------------------------------------

#[test]
fn delete_frames_remaps_keyframes() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);

    let mut layer = TextLayer::new("kf".to_string());
    layer.frame_range = (0, total - 1);
    layer.keyframes = vec![
        Keyframe {
            frame: 0,
            position: (0.0, 0.0),
            opacity: 1.0,
        },
        Keyframe {
            frame: 1,
            position: (10.0, 10.0),
            opacity: 0.5,
        },
        Keyframe {
            frame: 2,
            position: (20.0, 20.0),
            opacity: 0.0,
        },
    ];
    project.layers.push(Layer::Text(layer));

    // Delete frame 1 (source index 1); keyframe at frame 1 should be dropped,
    // keyframe at old frame 2 should remap to logical 1.
    project.delete_frames(&[1]).unwrap();

    let kfs = project.layers[0].keyframes();
    assert_eq!(kfs.len(), 2);
    assert_eq!(kfs[0].frame, 0);
    assert_eq!(kfs[0].position, (0.0, 0.0));
    assert_eq!(kfs[1].frame, 1);
    assert_eq!(kfs[1].position, (20.0, 20.0));
}

// ---------------------------------------------------------------------------
// restore_frames with layer keyframes
// ---------------------------------------------------------------------------

#[test]
fn restore_frames_remaps_keyframes() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);

    let mut layer = TextLayer::new("kf".to_string());
    layer.frame_range = (0, total - 1);
    layer.keyframes = vec![
        Keyframe {
            frame: 0,
            position: (0.0, 0.0),
            opacity: 1.0,
        },
        Keyframe {
            frame: 2,
            position: (20.0, 20.0),
            opacity: 0.0,
        },
    ];
    project.layers.push(Layer::Text(layer));

    // Delete frame 1, then restore it
    project.delete_frames(&[1]).unwrap();
    project.restore_frames(&[1]).unwrap();

    let kfs = project.layers[0].keyframes();
    assert_eq!(kfs.len(), 2);
    assert_eq!(kfs[0].frame, 0);
    assert_eq!(kfs[1].frame, 2);
}

// ---------------------------------------------------------------------------
// open with unsupported format
// ---------------------------------------------------------------------------

#[test]
fn open_unsupported_format_returns_error() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake.xyz");
    // Create a dummy file so the error is about format, not missing file
    std::fs::write(&path, b"not a real file").unwrap();
    let result = Project::open(&path);
    assert!(result.is_err());
    std::fs::remove_file(&path).ok();
}

// ---------------------------------------------------------------------------
// flip_layer
// ---------------------------------------------------------------------------

#[test]
fn flip_layer_horizontal_inverts_scale_x() {
    let mut project = open_test_gif();
    let layer = project.add_text_layer("test".to_string(), None, None, None, None);
    assert!((project.layers[0].scale_x_val() - 1.0).abs() < 1e-9);
    project.flip_layer(layer.id, "horizontal").unwrap();
    assert!((project.layers[0].scale_x_val() + 1.0).abs() < 1e-9);
}

#[test]
fn flip_layer_vertical_inverts_scale_y() {
    let mut project = open_test_gif();
    let layer = project.add_text_layer("test".to_string(), None, None, None, None);
    project.flip_layer(layer.id, "vertical").unwrap();
    assert!((project.layers[0].scale_y_val() + 1.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// duplicate_layer
// ---------------------------------------------------------------------------

#[test]
fn duplicate_layer_creates_new_uuid() {
    let mut project = open_test_gif();
    let layer = project.add_text_layer("original".to_string(), None, None, None, None);
    let dup = project.duplicate_layer(layer.id).unwrap();
    assert_ne!(dup.id, layer.id);
    assert_eq!(project.layers.len(), 2);
}

#[test]
fn duplicate_layer_inserts_above_source() {
    let mut project = open_test_gif();
    let layer = project.add_text_layer("original".to_string(), None, None, None, None);
    project.duplicate_layer(layer.id).unwrap();
    // After duplication, the duplicate is at index 1 (above source at index 0)
    assert_eq!(project.layers.len(), 2);
    // Duplicate is inserted after source (on top in rendering stack)
    assert_eq!(project.layers[0].id(), layer.id);
}

// ---------------------------------------------------------------------------
// source_indices
// ---------------------------------------------------------------------------

#[test]
fn source_indices_matches_per_index_mapping() {
    let mut project = open_test_gif();
    let expected_mapping = |p: &Project| -> Vec<usize> {
        (0..p.visible_frame_count())
            .filter_map(|li| p.logical_to_source(li))
            .collect()
    };

    // No exclusions: identity mapping over all frames.
    assert_eq!(project.source_indices(), expected_mapping(&project));
    assert_eq!(
        project.source_indices().len(),
        project.visible_frame_count()
    );

    // With exclusions: still equal to the per-index logical_to_source scan.
    project.delete_frames(&[1]).unwrap();
    let indices = project.source_indices();
    assert_eq!(indices, expected_mapping(&project));
    assert_eq!(indices.len(), project.visible_frame_count());
    assert!(!indices.contains(&1));
}
