use std::path::PathBuf;
use std::process::Command;

use gif_editor_lib::frame_source::FrameSource;
use gif_editor_lib::video_decoder::VideoData;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test.mp4")
}

fn ensure_test_video() {
    let path = fixture_path();
    if path.exists() {
        return;
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "color=c=blue:s=64x64:d=0.5,fps=10",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
        ])
        .arg(&path)
        .status()
        .expect("ffmpeg not found");
    assert!(status.success(), "ffmpeg failed to create test video");
}

#[test]
fn open_video_metadata() {
    ensure_test_video();
    let vd = VideoData::open(&fixture_path()).unwrap();
    assert_eq!(vd.dimensions(), (64, 64));
    assert_eq!(vd.frame_count(), 5); // 0.5s at 10fps
    assert_eq!(vd.delays().len(), 5);
    // Each delay should be 10cs (100ms) for 10fps
    assert_eq!(vd.delays()[0], 10);
}

#[test]
fn get_frame_returns_correct_dimensions() {
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    let frame = vd.get_frame(0).unwrap();
    assert_eq!(frame.width(), 64);
    assert_eq!(frame.height(), 64);
}

#[test]
fn get_frame_caches_on_second_access() {
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    let _f1 = vd.get_frame(0).unwrap();
    // Second access should hit cache
    let f2 = vd.get_frame(0).unwrap();
    assert_eq!(f2.width(), 64);
}

#[test]
fn get_frame_out_of_bounds() {
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    assert!(vd.get_frame(100).is_err());
}

#[test]
fn project_opens_video() {
    ensure_test_video();
    let (mut proj, meta) = gif_editor_lib::project::Project::open(&fixture_path()).unwrap();
    assert_eq!(meta.width, 64);
    assert_eq!(meta.height, 64);
    assert_eq!(meta.frame_count, 5);

    // Verify we can get a frame PNG
    let png_path = proj.get_frame_png_path(0).unwrap();
    assert!(std::path::Path::new(&png_path).exists());
}
