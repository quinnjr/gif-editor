use std::path::PathBuf;
use std::process::Command;

use gif_editor_lib::error::AppError;
use gif_editor_lib::export::ffmpeg_available;
use gif_editor_lib::frame_source::FrameSource;
use gif_editor_lib::video_decoder::VideoData;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test.mp4")
}

/// Skip guard mirroring the export_test pattern: returns true (after
/// printing a notice) when ffmpeg is not on PATH, so tests skip instead of
/// hard-failing on ffmpeg-less machines.
fn skip_without_ffmpeg(test: &str) -> bool {
    if ffmpeg_available() {
        return false;
    }
    eprintln!("skipping {test}: ffmpeg not on PATH");
    true
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
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=64x64:d=0.5,fps=10",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&path)
        .status()
        .expect("ffmpeg not found");
    assert!(status.success(), "ffmpeg failed to create test video");
}

#[test]
fn open_video_metadata() {
    if skip_without_ffmpeg("open_video_metadata") {
        return;
    }
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
    if skip_without_ffmpeg("get_frame_returns_correct_dimensions") {
        return;
    }
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    let frame = vd.get_frame(0).unwrap();
    assert_eq!(frame.width(), 64);
    assert_eq!(frame.height(), 64);
}

#[test]
fn get_frame_caches_on_second_access() {
    if skip_without_ffmpeg("get_frame_caches_on_second_access") {
        return;
    }
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    let _f1 = vd.get_frame(0).unwrap();
    // Second access should hit cache
    let f2 = vd.get_frame(0).unwrap();
    assert_eq!(f2.width(), 64);
}

#[test]
fn get_frame_out_of_bounds() {
    if skip_without_ffmpeg("get_frame_out_of_bounds") {
        return;
    }
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    assert!(vd.get_frame(100).is_err());
}

#[test]
fn stream_frames_decodes_all_frames_in_order() {
    if skip_without_ffmpeg("stream_frames_decodes_all_frames_in_order") {
        return;
    }
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    // Frame 0 via the per-frame seek path, for comparison below.
    let seeked_frame0 = vd.get_frame(0).unwrap();

    let mut seen = Vec::new();
    let mut streamed_frame0 = None;
    vd.stream_frames(vd.frame_count(), |i, frame| {
        assert_eq!((frame.width(), frame.height()), (64, 64));
        if i == 0 {
            streamed_frame0 = Some(frame);
        }
        seen.push(i);
        Ok(())
    })
    .unwrap();

    assert_eq!(seen, vec![0, 1, 2, 3, 4]);
    // Streaming and seeking must decode identical pixels.
    assert_eq!(streamed_frame0.unwrap().as_raw(), seeked_frame0.as_raw());
}

#[test]
fn stream_frames_stops_at_up_to() {
    if skip_without_ffmpeg("stream_frames_stops_at_up_to") {
        return;
    }
    ensure_test_video();
    let vd = VideoData::open(&fixture_path()).unwrap();
    let mut seen = Vec::new();
    vd.stream_frames(2, |i, _| {
        seen.push(i);
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, vec![0, 1]);
}

#[test]
fn stream_frames_propagates_callback_error() {
    if skip_without_ffmpeg("stream_frames_propagates_callback_error") {
        return;
    }
    ensure_test_video();
    let vd = VideoData::open(&fixture_path()).unwrap();
    let mut seen = Vec::new();
    let result = vd.stream_frames(5, |i, _| {
        seen.push(i);
        if i == 1 {
            Err(AppError::VideoDecode("callback abort".to_string()))
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    assert_eq!(seen, vec![0, 1]);
}

#[test]
fn project_opens_video() {
    if skip_without_ffmpeg("project_opens_video") {
        return;
    }
    ensure_test_video();
    let (mut proj, meta) = gif_editor_lib::project::Project::open(&fixture_path()).unwrap();
    assert_eq!(meta.width, 64);
    assert_eq!(meta.height, 64);
    assert_eq!(meta.frame_count, 5);

    // Verify we can get a frame PNG
    let png_path = proj.get_frame_png_path(0).unwrap();
    assert!(std::path::Path::new(&png_path).exists());
}

#[test]
fn open_nonexistent_file_returns_error() {
    // No ffmpeg guard needed: VideoData::open checks existence before
    // invoking ffprobe.
    let bogus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("does_not_exist.mp4");
    let result = VideoData::open(&bogus);
    assert!(result.is_err());
}

#[test]
fn video_source_path() {
    if skip_without_ffmpeg("video_source_path") {
        return;
    }
    ensure_test_video();
    let vd = VideoData::open(&fixture_path()).unwrap();
    assert_eq!(vd.source_path(), fixture_path().as_path());
}

#[test]
fn video_as_any_mut() {
    if skip_without_ffmpeg("video_as_any_mut") {
        return;
    }
    ensure_test_video();
    let mut vd = VideoData::open(&fixture_path()).unwrap();
    let any = vd.as_any_mut();
    assert!(any.downcast_mut::<VideoData>().is_some());
}
