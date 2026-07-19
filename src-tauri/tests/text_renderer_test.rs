use gif_editor_lib::layer::{Stroke, TextLayer};
use gif_editor_lib::text_renderer::{clear_render_cache, render_text, stroke_pad};

// Font metrics for the bundled fonts (from their hhea/head tables),
// used to compute expected pixel sizes under CSS-px em-size semantics.
// Anton-Regular.ttf:        ascent 2409, descent -674, units_per_em 2048
// LiberationSans-Bold.ttf:  ascent 1854, descent -434, units_per_em 2048
const ANTON_HEIGHT_PER_EM: f64 = 3083.0 / 2048.0; // ≈ 1.5054
const LIBERATION_HEIGHT_PER_EM: f64 = 2288.0 / 2048.0; // ≈ 1.1172

#[test]
fn render_text_produces_non_empty_image() {
    let layer = TextLayer::new("Hello".to_string());
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
    assert!(result.height() > 0);
    let has_content = result.pixels().any(|p| p[3] > 0);
    assert!(has_content, "Rendered text should have visible pixels");
}

#[test]
fn render_text_with_stroke() {
    let mut layer = TextLayer::new("Meme".to_string());
    layer.stroke = Some(gif_editor_lib::layer::Stroke {
        color: [0, 0, 0, 255],
        width: 3.0,
    });
    let stroked = render_text(&layer).unwrap();

    // The stroke pad must make the image strictly larger than the same
    // layer rendered without a stroke, in both dimensions — a stroke that
    // is silently ignored would produce identical dimensions.
    let mut plain = layer.clone();
    plain.stroke = None;
    let no_stroke = render_text(&plain).unwrap();
    assert!(
        stroked.width() > no_stroke.width(),
        "stroked width {} must exceed no-stroke width {}",
        stroked.width(),
        no_stroke.width()
    );
    assert!(
        stroked.height() > no_stroke.height(),
        "stroked height {} must exceed no-stroke height {}",
        stroked.height(),
        no_stroke.height()
    );
}

#[test]
fn render_empty_text_returns_empty_image() {
    let layer = TextLayer::new(String::new());
    let result = render_text(&layer).unwrap();
    assert!(result.width() <= 1 || result.height() <= 1);
}

#[test]
fn render_text_with_large_stroke_width() {
    // Large stroke width (> 2.0) triggers the 16-sample offset path
    let mut layer = TextLayer::new("Big".to_string());
    layer.stroke = Some(Stroke {
        color: [255, 0, 0, 255],
        width: 8.0,
    });
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
    assert!(result.height() > 0);
    // The image must be strictly larger than without stroke: the stroke pad
    // (ceil(width) + 2) is guaranteed positive for any Some(stroke).
    let no_stroke = {
        let mut l = TextLayer::new("Big".to_string());
        l.stroke = None;
        render_text(&l).unwrap()
    };
    assert!(result.width() > no_stroke.width());
    assert!(result.height() > no_stroke.height());
}

#[test]
fn render_text_with_zero_width_stroke() {
    // A Some(zero-width) stroke draws no stroke offsets, but it still pads
    // the image by stroke_pad = ceil(0) + 2 = 2 on every side: the pad is a
    // property of having a stroke configured, not of its width.
    let mut layer = TextLayer::new("Zero".to_string());
    layer.stroke = Some(Stroke {
        color: [0, 0, 0, 255],
        width: 0.0,
    });
    let pad = stroke_pad(&layer);
    assert_eq!(pad, 2, "ceil(0.0) + 2");

    let result = render_text(&layer).unwrap();
    let mut plain = layer.clone();
    plain.stroke = None;
    let no_stroke = render_text(&plain).unwrap();
    assert_eq!(result.width(), no_stroke.width() + 2 * pad);
    assert_eq!(result.height(), no_stroke.height() + 2 * pad);
}

#[test]
fn render_text_different_font_sizes() {
    let mut small = TextLayer::new("A".to_string());
    small.font_size = 12.0;
    let small_img = render_text(&small).unwrap();

    let mut large = TextLayer::new("A".to_string());
    large.font_size = 96.0;
    let large_img = render_text(&large).unwrap();

    assert!(large_img.width() > small_img.width());
    assert!(large_img.height() > small_img.height());
}

#[test]
fn word_wrap_splits_long_text() {
    let mut layer = TextLayer::new("word1 word2 word3 word4".to_string());
    layer.font_size = 24.0;
    layer.max_width = Some(60.0); // force wrap after ~2 words
    let result = render_text(&layer).unwrap();
    // A wrapped image should be taller than a single-line image of the same text.
    let mut single_line = TextLayer::new("word1 word2 word3 word4".to_string());
    single_line.font_size = 24.0;
    let single = render_text(&single_line).unwrap();
    assert!(
        result.height() > single.height(),
        "Wrapped text should be taller"
    );
}

#[test]
fn center_aligned_wider_than_single_character() {
    let mut layer = TextLayer::new("Hello World".to_string());
    layer.font_size = 32.0;
    layer.text_align = "center".to_string();
    layer.max_width = Some(200.0);
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
    assert!(result.height() > 0);
}

/// font_size must mean CSS-px em size (what `ctx.font = "100px ..."` means
/// in the preview), NOT the ab_glyph ascent−descent height.  A single-line,
/// no-stroke image is exactly ceil(ascent − descent) tall, which under em
/// semantics is font_size × height_unscaled / units_per_em.
#[test]
fn font_size_uses_css_px_em_semantics() {
    let mut layer = TextLayer::new("Hg".to_string());
    layer.font_size = 100.0;
    layer.stroke = None;
    layer.font_family = "Impact".to_string(); // served by Anton
    let img = render_text(&layer).unwrap();

    let expected = (100.0 * ANTON_HEIGHT_PER_EM).ceil() as i64; // 151, not 100
    assert!(
        (img.height() as i64 - expected).abs() <= 2,
        "Anton at 100px should be ~{expected}px tall (em semantics), got {}",
        img.height()
    );

    let mut lib = TextLayer::new("Hg".to_string());
    lib.font_size = 100.0;
    lib.stroke = None;
    lib.font_family = "Arial".to_string(); // falls back to Liberation Sans Bold
    let lib_img = render_text(&lib).unwrap();
    let expected = (100.0 * LIBERATION_HEIGHT_PER_EM).ceil() as i64; // 112
    assert!(
        (lib_img.height() as i64 - expected).abs() <= 2,
        "Liberation at 100px should be ~{expected}px tall (em semantics), got {}",
        lib_img.height()
    );
}

/// Line advance must match the preview: fontSize * 1.2 CSS px per line.
/// Each extra wrapped line adds exactly one line_height to the image.
#[test]
fn line_height_is_1_2_times_font_size() {
    let mut two_lines = TextLayer::new("aaaa aaaa".to_string());
    two_lines.font_size = 100.0;
    two_lines.stroke = None;
    two_lines.max_width = Some(10.0); // forces one word per line
    let two = render_text(&two_lines).unwrap();

    let mut one_line = TextLayer::new("aaaa".to_string());
    one_line.font_size = 100.0;
    one_line.stroke = None;
    one_line.max_width = Some(10.0);
    let one = render_text(&one_line).unwrap();

    let diff = two.height() as i64 - one.height() as i64;
    assert!(
        (diff - 120).abs() <= 1,
        "second line should add fontSize*1.2 = 120px, added {diff}px"
    );
}

/// The stroke pad must surround the glyph box symmetrically so the
/// compositor can anchor the glyph box (not the pad edge) on the layer
/// position by offsetting placement by -stroke_pad().
#[test]
fn stroke_pad_adds_symmetric_margin() {
    let mut stroked = TextLayer::new("Pad".to_string());
    stroked.stroke = Some(Stroke {
        color: [0, 0, 0, 255],
        width: 3.0,
    });
    let pad = stroke_pad(&stroked);
    assert_eq!(pad, 5, "ceil(3.0) + 2");

    let mut plain = stroked.clone();
    plain.stroke = None;
    assert_eq!(stroke_pad(&plain), 0);

    let s_img = render_text(&stroked).unwrap();
    let p_img = render_text(&plain).unwrap();
    assert_eq!(s_img.width(), p_img.width() + 2 * pad);
    assert_eq!(s_img.height(), p_img.height() + 2 * pad);
}

/// Golden text-metric test: the rendered line width of a fixed string at a
/// fixed size with the bundled Anton face must stay stable.  The literal was
/// captured from a known-good run; ±1 px of tolerance absorbs rounding
/// differences across ab_glyph releases.
///
/// There is deliberately no TypeScript twin for this golden: jsdom mocks
/// canvas measureText, and real browser (HarfBuzz) metrics differ from
/// ab_glyph's, so cross-engine golden values are not meaningful.  The
/// backend↔preview parity contract is instead anchored on em-size semantics
/// (see the css_px_scale tests: font_size_uses_css_px_em_semantics and
/// line_height_is_1_2_times_font_size).
#[test]
fn golden_line_width_hello_anton_100px() {
    let mut layer = TextLayer::new("Hello".to_string());
    layer.font_size = 100.0;
    layer.stroke = None;
    layer.max_width = None;
    layer.font_family = "Impact".to_string(); // served by Anton
    let img = render_text(&layer).unwrap();

    // Captured golden: rendered single-line image width (== measured line
    // width, no stroke pad) for "Hello" at 100px with Anton-Regular.ttf.
    const GOLDEN_WIDTH: i64 = 198;
    assert!(
        (img.width() as i64 - GOLDEN_WIDTH).abs() <= 1,
        "\"Hello\" at 100px Anton should measure ~{GOLDEN_WIDTH}px wide, got {}",
        img.width()
    );
}

/// Two consecutive renders of an identical layer must hit the render cache
/// (observable as the very same Arc allocation) and therefore return
/// identical pixels; changing any content-affecting field must miss.
#[test]
fn render_text_caches_identical_layers() {
    // The render cache is process-global and capped at 16 entries, and the
    // other tests in this binary run on parallel threads.  Start from an
    // empty cache and keep each probe render adjacent to its re-render so an
    // eviction between the paired calls would need 16+ concurrent inserts in
    // a few microseconds — theoretically possible, practically never.
    // (Deliberately not serialised: that would need a new dev-dependency.)
    clear_render_cache();

    let mut layer = TextLayer::new("CacheProbe".to_string());
    layer.font_size = 33.0;
    let first = render_text(&layer).unwrap();
    let second = render_text(&layer).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "identical layers must return the cached image"
    );
    assert_eq!(*first, *second, "cached pixels must match");

    // Transform-only fields are applied after rasterisation and must NOT
    // affect the cache key.
    let mut moved = layer.clone();
    moved.position = (123.0, 45.0);
    moved.opacity = 0.5;
    let third = render_text(&moved).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&first, &third),
        "position/opacity changes must still hit the cache"
    );

    // A changed content field must miss the cache and re-rasterise.
    let mut recolored = layer.clone();
    recolored.color = [10, 200, 30, 255];
    let fourth = render_text(&recolored).unwrap();
    assert!(
        !std::sync::Arc::ptr_eq(&first, &fourth),
        "a changed content field must miss the cache"
    );
}

#[test]
fn renders_with_impact_family_uses_anton_font() {
    // "Impact" is now served by Anton.ttf; should render without error.
    let mut layer = TextLayer::new("MEME".to_string());
    layer.font_family = "Impact".to_string();
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
}
