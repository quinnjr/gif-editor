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
                gif::Encoder::new(std::fs::File::create(&tmp_path).unwrap(), 10, 10, &[])
                    .unwrap();
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
