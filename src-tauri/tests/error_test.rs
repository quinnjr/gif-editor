use gif_editor_lib::error::AppError;

#[test]
fn app_error_serializes() {
    let err = AppError::NoProject;
    let json = serde_json::to_string(&err).unwrap();
    assert_eq!(json, "\"No project open\"");
}

#[test]
fn app_error_gif_decode_serializes() {
    let err = AppError::GifDecode("bad header".to_string());
    let json = serde_json::to_string(&err).unwrap();
    assert_eq!(json, "\"Failed to decode GIF: bad header\"");
}
