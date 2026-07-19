use gif_editor_lib::frame_source::FrameSource;
use gif_editor_lib::gif_decoder::GifData;
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("test.gif")
}

fn ensure_test_gif() {
    use std::sync::OnceLock;
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let path = fixture_path();
        if path.exists() {
            return;
        }
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        // Write to a temp file then rename so readers never see a partial file.
        let tmp_path = dir.join("test.gif.tmp");
        {
            let mut encoder =
                gif::Encoder::new(std::fs::File::create(&tmp_path).unwrap(), 10, 10, &[]).unwrap();
            encoder.set_repeat(gif::Repeat::Infinite).unwrap();
            for i in 0u8..3 {
                let pixels: Vec<u8> = (0..100).flat_map(|_| [i * 80, 0, 0, 255]).collect();
                let mut frame = gif::Frame::from_rgba(10, 10, &mut pixels.clone());
                frame.delay = 10;
                encoder.write_frame(&frame).unwrap();
            }
        } // encoder flushed/closed here
        std::fs::rename(&tmp_path, &path).unwrap();
    });
}

// ---------------------------------------------------------------------------
// Delta-optimized fixtures (frame disposal handling)
//
// Real-world GIFs carry only changed sub-rectangles per frame and rely on
// disposal semantics for correct compositing. These fixtures are generated
// programmatically so the expected pixel values are known exactly.
// ---------------------------------------------------------------------------

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

/// Description of one frame in a generated fixture: sub-rect position/size,
/// RGBA pixel data for the rect, and the disposal method.
struct FrameSpec {
    left: u16,
    top: u16,
    width: u16,
    height: u16,
    pixels: Vec<u8>,
    dispose: gif::DisposalMethod,
}

fn solid_rect(width: u16, height: u16, rgba: [u8; 4]) -> Vec<u8> {
    (0..width as usize * height as usize)
        .flat_map(|_| rgba)
        .collect()
}

/// Writes a delta-optimized GIF fixture atomically (temp file + rename) so
/// concurrently running tests never observe a partial file. Callers serialize
/// per-fixture generation with a OnceLock.
fn write_delta_fixture(name: &str, canvas_w: u16, canvas_h: u16, frames: &[FrameSpec]) -> PathBuf {
    let path = fixture_path().with_file_name(name);
    if path.exists() {
        return path;
    }
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let tmp_path = path.with_extension("gif.tmp");
    {
        let mut encoder = gif::Encoder::new(
            std::fs::File::create(&tmp_path).unwrap(),
            canvas_w,
            canvas_h,
            &[],
        )
        .unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        for spec in frames {
            let mut pixels = spec.pixels.clone();
            let mut frame = gif::Frame::from_rgba(spec.width, spec.height, &mut pixels);
            frame.left = spec.left;
            frame.top = spec.top;
            frame.delay = 10;
            frame.dispose = spec.dispose;
            encoder.write_frame(&frame).unwrap();
        }
    }
    std::fs::rename(&tmp_path, &path).unwrap();
    path
}

/// 3-frame GIF, 10x10, all frames use disposal Keep:
/// - frame 0: full-canvas red
/// - frame 1: 4x4 green rect at (2,2) whose top-left pixel is transparent
/// - frame 2: 2x2 blue rect at (6,6)
fn keep_fixture() -> PathBuf {
    static ONCE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ONCE.get_or_init(build_keep_fixture).clone()
}

fn build_keep_fixture() -> PathBuf {
    let mut green = solid_rect(4, 4, GREEN);
    green[0..4].copy_from_slice(&TRANSPARENT); // rect-local (0,0) transparent
    write_delta_fixture(
        "delta_keep.gif",
        10,
        10,
        &[
            FrameSpec {
                left: 0,
                top: 0,
                width: 10,
                height: 10,
                pixels: solid_rect(10, 10, RED),
                dispose: gif::DisposalMethod::Keep,
            },
            FrameSpec {
                left: 2,
                top: 2,
                width: 4,
                height: 4,
                pixels: green,
                dispose: gif::DisposalMethod::Keep,
            },
            FrameSpec {
                left: 6,
                top: 6,
                width: 2,
                height: 2,
                pixels: solid_rect(2, 2, BLUE),
                dispose: gif::DisposalMethod::Keep,
            },
        ],
    )
}

/// 3-frame GIF, 10x10, exercising Background disposal:
/// - frame 0: full-canvas red, disposal Background (rect cleared afterwards)
/// - frame 1: 4x4 green rect at (0,0), disposal Keep
/// - frame 2: 2x2 blue rect at (5,5), disposal Keep
fn background_fixture() -> PathBuf {
    static ONCE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ONCE.get_or_init(build_background_fixture).clone()
}

fn build_background_fixture() -> PathBuf {
    write_delta_fixture(
        "delta_background.gif",
        10,
        10,
        &[
            FrameSpec {
                left: 0,
                top: 0,
                width: 10,
                height: 10,
                pixels: solid_rect(10, 10, RED),
                dispose: gif::DisposalMethod::Background,
            },
            FrameSpec {
                left: 0,
                top: 0,
                width: 4,
                height: 4,
                pixels: solid_rect(4, 4, GREEN),
                dispose: gif::DisposalMethod::Keep,
            },
            FrameSpec {
                left: 5,
                top: 5,
                width: 2,
                height: 2,
                pixels: solid_rect(2, 2, BLUE),
                dispose: gif::DisposalMethod::Keep,
            },
        ],
    )
}

/// 3-frame GIF, 10x10, exercising Previous disposal:
/// - frame 0: full-canvas red, disposal Keep
/// - frame 1: 4x4 green rect at (2,2), disposal Previous (reverted afterwards)
/// - frame 2: 2x2 blue rect at (6,6), disposal Keep
fn previous_fixture() -> PathBuf {
    static ONCE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ONCE.get_or_init(build_previous_fixture).clone()
}

fn build_previous_fixture() -> PathBuf {
    write_delta_fixture(
        "delta_previous.gif",
        10,
        10,
        &[
            FrameSpec {
                left: 0,
                top: 0,
                width: 10,
                height: 10,
                pixels: solid_rect(10, 10, RED),
                dispose: gif::DisposalMethod::Keep,
            },
            FrameSpec {
                left: 2,
                top: 2,
                width: 4,
                height: 4,
                pixels: solid_rect(4, 4, GREEN),
                dispose: gif::DisposalMethod::Previous,
            },
            FrameSpec {
                left: 6,
                top: 6,
                width: 2,
                height: 2,
                pixels: solid_rect(2, 2, BLUE),
                dispose: gif::DisposalMethod::Keep,
            },
        ],
    )
}

fn px(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    img.get_pixel(x, y).0
}

#[test]
fn keep_disposal_composites_over_previous_frames() {
    let mut gif = GifData::open(&keep_fixture()).unwrap();

    // Frame 0: full red.
    let f0 = gif.get_frame(0).unwrap();
    assert_eq!(px(&f0, 0, 0), RED);
    assert_eq!(px(&f0, 9, 9), RED);

    // Frame 1: red background with green rect at (2,2)-(5,5); the rect's
    // transparent top-left pixel lets frame 0's red show through.
    let f1 = gif.get_frame(1).unwrap();
    assert_eq!(px(&f1, 0, 0), RED, "outside rect keeps previous frame");
    assert_eq!(px(&f1, 2, 2), RED, "transparent rect pixel shows previous");
    assert_eq!(px(&f1, 3, 3), GREEN);
    assert_eq!(px(&f1, 5, 5), GREEN);
    assert_eq!(px(&f1, 6, 6), RED);

    // Frame 2: previous composite plus blue rect at (6,6)-(7,7).
    let f2 = gif.get_frame(2).unwrap();
    assert_eq!(px(&f2, 0, 0), RED);
    assert_eq!(px(&f2, 3, 3), GREEN);
    assert_eq!(px(&f2, 6, 6), BLUE);
    assert_eq!(px(&f2, 7, 7), BLUE);
    assert_eq!(px(&f2, 8, 8), RED);
}

#[test]
fn background_disposal_clears_frame_rect() {
    let mut gif = GifData::open(&background_fixture()).unwrap();

    // Frame 0 itself is full red.
    let f0 = gif.get_frame(0).unwrap();
    assert_eq!(px(&f0, 0, 0), RED);
    assert_eq!(px(&f0, 9, 9), RED);

    // Frame 0's Background disposal clears its (full-canvas) rect before
    // frame 1 draws, so only the green rect is visible.
    let f1 = gif.get_frame(1).unwrap();
    assert_eq!(px(&f1, 0, 0), GREEN);
    assert_eq!(px(&f1, 3, 3), GREEN);
    assert_eq!(px(&f1, 5, 5), TRANSPARENT, "cleared area stays transparent");
    assert_eq!(px(&f1, 9, 9), TRANSPARENT);

    // Frame 2: green rect kept, blue rect added, rest still transparent.
    let f2 = gif.get_frame(2).unwrap();
    assert_eq!(px(&f2, 0, 0), GREEN);
    assert_eq!(px(&f2, 5, 5), BLUE);
    assert_eq!(px(&f2, 6, 6), BLUE);
    assert_eq!(px(&f2, 9, 9), TRANSPARENT);
}

#[test]
fn previous_disposal_restores_canvas() {
    let mut gif = GifData::open(&previous_fixture()).unwrap();

    // Frame 1 shows the green rect over red.
    let f1 = gif.get_frame(1).unwrap();
    assert_eq!(px(&f1, 3, 3), GREEN);
    assert_eq!(px(&f1, 0, 0), RED);

    // Frame 1's Previous disposal reverts the canvas to frame 0's state, so
    // frame 2 must show red (no green) plus the blue rect.
    let f2 = gif.get_frame(2).unwrap();
    assert_eq!(
        px(&f2, 3, 3),
        RED,
        "green rect reverted by Previous disposal"
    );
    assert_eq!(px(&f2, 6, 6), BLUE);
    assert_eq!(px(&f2, 0, 0), RED);
}

#[test]
fn random_access_matches_sequential_decoding() {
    // Request frame 2 first, then frame 1: results must equal sequential decode.
    let mut sequential = GifData::open(&keep_fixture()).unwrap();
    let seq0 = sequential.get_frame(0).unwrap();
    let seq1 = sequential.get_frame(1).unwrap();
    let seq2 = sequential.get_frame(2).unwrap();

    let mut random = GifData::open_with_cache_cap(&keep_fixture(), 1).unwrap();
    let r2 = random.get_frame(2).unwrap();
    let r1 = random.get_frame(1).unwrap();
    let r0 = random.get_frame(0).unwrap();

    assert_eq!(r0.as_raw(), seq0.as_raw());
    assert_eq!(r1.as_raw(), seq1.as_raw());
    assert_eq!(r2.as_raw(), seq2.as_raw());
}

#[test]
fn repeated_access_through_lru_is_stable() {
    // Cache cap 1 forces evictions and re-decodes between accesses.
    let mut gif = GifData::open_with_cache_cap(&previous_fixture(), 1).unwrap();
    let first = gif.get_frame(2).unwrap();
    gif.get_frame(0).unwrap(); // evicts frame 2
    let second = gif.get_frame(2).unwrap(); // re-decoded from scratch
    assert_eq!(first.as_raw(), second.as_raw());

    // And with the default cache: repeated hits return identical data.
    let mut cached = GifData::open(&background_fixture()).unwrap();
    let a = cached.get_frame(1).unwrap();
    let b = cached.get_frame(1).unwrap();
    assert_eq!(a.as_raw(), b.as_raw());
}

#[test]
fn delta_fixture_metadata() {
    let gif = GifData::open(&keep_fixture()).unwrap();
    assert_eq!(gif.frame_count(), 3);
    assert_eq!(gif.dimensions(), (10, 10));
    assert_eq!(gif.delays(), &[10, 10, 10]);
}

#[test]
fn decode_gif_metadata() {
    ensure_test_gif();
    let gif = GifData::open(&fixture_path()).unwrap();
    assert_eq!(gif.frame_count(), 3);
    assert_eq!(gif.dimensions(), (10, 10));
    assert_eq!(gif.delays().len(), 3);
}

#[test]
fn get_frame_returns_rgba_image() {
    ensure_test_gif();
    let mut gif = GifData::open(&fixture_path()).unwrap();
    let frame = gif.get_frame(0).unwrap();
    assert_eq!(frame.dimensions(), (10, 10));
}

#[test]
fn get_frame_out_of_bounds() {
    ensure_test_gif();
    let mut gif = GifData::open(&fixture_path()).unwrap();
    assert!(gif.get_frame(99).is_err());
}

#[test]
fn lru_cache_evicts_old_frames() {
    ensure_test_gif();
    let mut gif = GifData::open_with_cache_cap(&fixture_path(), 2).unwrap();
    gif.get_frame(0).unwrap();
    gif.get_frame(1).unwrap();
    gif.get_frame(2).unwrap(); // evicts frame 0
    let frame0 = gif.get_frame(0).unwrap(); // re-decoded
    assert_eq!(frame0.dimensions(), (10, 10));
}

// ---------------------------------------------------------------------------
// FrameSource trait method coverage
// ---------------------------------------------------------------------------

#[test]
fn gif_data_source_path() {
    ensure_test_gif();
    let gif = GifData::open(&fixture_path()).unwrap();
    assert_eq!(gif.source_path(), fixture_path().as_path());
}

#[test]
fn gif_data_delays_values() {
    ensure_test_gif();
    let gif = GifData::open(&fixture_path()).unwrap();
    let delays = gif.delays();
    assert_eq!(delays.len(), 3);
    // All frames in the test fixture have delay=10
    for &d in delays {
        assert_eq!(d, 10);
    }
}

#[test]
fn gif_data_get_frame_caching() {
    ensure_test_gif();
    let mut gif = GifData::open(&fixture_path()).unwrap();
    // First call decodes and caches
    let f1 = gif.get_frame(1).unwrap();
    // Second call should hit cache and return identical data
    let f2 = gif.get_frame(1).unwrap();
    assert_eq!(f1.dimensions(), f2.dimensions());
    assert_eq!(f1.as_raw(), f2.as_raw());
}

#[test]
fn gif_data_as_any_mut() {
    ensure_test_gif();
    let mut gif = GifData::open(&fixture_path()).unwrap();
    let any = gif.as_any_mut();
    assert!(any.downcast_mut::<GifData>().is_some());
}

#[test]
fn gif_data_frame_source_get_frame() {
    ensure_test_gif();
    let mut gif = GifData::open(&fixture_path()).unwrap();
    // Call via the FrameSource trait
    let frame: image::RgbaImage = FrameSource::get_frame(&mut gif, 0).unwrap();
    assert_eq!(frame.dimensions(), (10, 10));
}
