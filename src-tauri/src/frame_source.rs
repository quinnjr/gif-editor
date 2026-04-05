// FrameSource trait — abstracts over GIF and video decoders so the rest of
// the app (project, compositor, export) can work with any supported input
// format through a single interface.

use std::any::Any;
use std::path::Path;

use image::RgbaImage;

use crate::error::AppError;

/// Uniform interface for accessing decoded frames from any supported source
/// format (GIF, MP4, WebM, static image).
pub trait FrameSource: Send {
    fn frame_count(&self) -> usize;
    fn dimensions(&self) -> (u32, u32);
    fn delays(&self) -> &[u16];
    fn source_path(&self) -> &Path;
    fn get_frame(&mut self, index: usize) -> Result<RgbaImage, AppError>;

    /// Downcast helper for concrete-type access (e.g. ImageSource::expand_timeline).
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
