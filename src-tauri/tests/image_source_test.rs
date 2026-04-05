use std::path::Path;

use gif_editor_lib::frame_source::FrameSource;
use gif_editor_lib::image_source::ImageSource;

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test.png")
}

#[test]
fn open_png_returns_single_frame() {
    let src = ImageSource::open(&fixture_path()).unwrap();
    assert_eq!(src.frame_count(), 1);
}

#[test]
fn open_png_dimensions() {
    let src = ImageSource::open(&fixture_path()).unwrap();
    assert_eq!(src.dimensions(), (2, 2));
}

#[test]
fn open_png_delays_has_single_element() {
    let src = ImageSource::open(&fixture_path()).unwrap();
    let delays = src.delays();
    assert_eq!(delays.len(), 1);
    assert_eq!(delays[0], 10); // default 100ms
}

#[test]
fn open_png_source_path() {
    let path = fixture_path();
    let src = ImageSource::open(&path).unwrap();
    assert_eq!(src.source_path(), path.as_path());
}

#[test]
fn get_frame_zero_succeeds() {
    let mut src = ImageSource::open(&fixture_path()).unwrap();
    let frame = src.get_frame(0).unwrap();
    assert_eq!(frame.dimensions(), (2, 2));
}

#[test]
fn get_frame_one_fails_for_single_frame() {
    let mut src = ImageSource::open(&fixture_path()).unwrap();
    let result = src.get_frame(1);
    assert!(result.is_err());
}

#[test]
fn expand_timeline_increases_frame_count() {
    let mut src = ImageSource::open(&fixture_path()).unwrap();
    assert_eq!(src.frame_count(), 1);
    src.expand_timeline(10, 5);
    assert_eq!(src.frame_count(), 10);
    // Delay should be updated
    assert_eq!(src.delays()[0], 5);
}

#[test]
fn get_frame_works_after_expansion() {
    let mut src = ImageSource::open(&fixture_path()).unwrap();
    src.expand_timeline(5, 8);
    // All expanded frames return the same image
    for i in 0..5 {
        let frame = src.get_frame(i).unwrap();
        assert_eq!(frame.dimensions(), (2, 2));
    }
    // Beyond expanded count still fails
    assert!(src.get_frame(5).is_err());
}

#[test]
fn expand_timeline_no_op_when_smaller() {
    let mut src = ImageSource::open(&fixture_path()).unwrap();
    src.expand_timeline(10, 5);
    assert_eq!(src.frame_count(), 10);
    // Expanding to a smaller count should not shrink
    src.expand_timeline(3, 20);
    assert_eq!(src.frame_count(), 10);
    assert_eq!(src.delays()[0], 5); // delay unchanged
}

#[test]
fn as_any_mut_returns_self() {
    let mut src = ImageSource::open(&fixture_path()).unwrap();
    let any = src.as_any_mut();
    assert!(any.downcast_mut::<ImageSource>().is_some());
}
