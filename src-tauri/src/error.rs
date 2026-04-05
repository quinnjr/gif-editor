use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("No project open")]
    NoProject,
    #[error("Failed to decode GIF: {0}")]
    GifDecode(String),
    #[error("Failed to decode video: {0}")]
    VideoDecode(String),
    #[error("Failed to load image: {0}")]
    ImageLoad(String),
    #[error("Layer not found: {0}")]
    LayerNotFound(uuid::Uuid),
    #[error("Frame deletion error: {0}")]
    FrameDeletion(String),
    #[error("Export failed: {0}")]
    Export(String),
    #[error("Font error: {0}")]
    Font(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
