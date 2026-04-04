use gif_editor_lib::text_renderer::render_text;
use gif_editor_lib::layer::TextLayer;

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
    layer.stroke = Some(gif_editor_lib::layer::Stroke { color: [0, 0, 0, 255], width: 3.0 });
    let result = render_text(&layer).unwrap();
    assert!(result.width() > 0);
}

#[test]
fn render_empty_text_returns_empty_image() {
    let layer = TextLayer::new(String::new());
    let result = render_text(&layer).unwrap();
    assert!(result.width() <= 1 || result.height() <= 1);
}
