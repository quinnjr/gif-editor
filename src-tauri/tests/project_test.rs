use gif_editor_lib::layer::{Layer, TextLayer};
use gif_editor_lib::project::Project;
use std::path::Path;

fn open_test_gif() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test.gif");
    let (project, _meta) = Project::open(&path).unwrap();
    project
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
