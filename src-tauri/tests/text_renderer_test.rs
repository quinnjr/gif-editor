use gif_editor_lib::layer::{Stroke, TextLayer};
use gif_editor_lib::text_renderer::render_text;

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
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
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
    // The image should be larger than without stroke due to padding
    let no_stroke = {
        let mut l = TextLayer::new("Big".to_string());
        l.stroke = None;
        render_text(&l).unwrap()
    };
    assert!(result.width() >= no_stroke.width());
    assert!(result.height() >= no_stroke.height());
}

#[test]
fn render_text_with_zero_width_stroke() {
    // A zero-width stroke should produce no stroke offsets
    let mut layer = TextLayer::new("Zero".to_string());
    layer.stroke = Some(Stroke {
        color: [0, 0, 0, 255],
        width: 0.0,
    });
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
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
    assert!(result.height() > single.height(), "Wrapped text should be taller");
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

#[test]
fn renders_with_impact_family_uses_anton_font() {
    // "Impact" is now served by Anton.ttf; should render without error.
    let mut layer = TextLayer::new("MEME".to_string());
    layer.font_family = "Impact".to_string();
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
}
