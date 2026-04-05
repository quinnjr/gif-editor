// Static image source — treats a single PNG/JPEG as a one-frame project.
//
// This lets users open a static image and layer animated GIFs, text, or
// other images on top of it.

use std::path::{Path, PathBuf};

use image::RgbaImage;

use crate::error::AppError;
use crate::frame_source::FrameSource;

pub struct ImageSource {
    source_path: PathBuf,
    image: RgbaImage,
}

impl ImageSource {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let image = image::open(path)
            .map_err(|e| AppError::ImageLoad(e.to_string()))?
            .to_rgba8();
        Ok(Self {
            source_path: path.to_path_buf(),
            image,
        })
    }
}

impl FrameSource for ImageSource {
    fn frame_count(&self) -> usize {
        1
    }

    fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    fn delays(&self) -> &[u16] {
        // Single frame with a nominal 1-second delay (100 centiseconds).
        &[100]
    }

    fn source_path(&self) -> &Path {
        &self.source_path
    }

    fn get_frame(&mut self, index: usize) -> Result<RgbaImage, AppError> {
        if index == 0 {
            Ok(self.image.clone())
        } else {
            Err(AppError::ImageLoad(format!(
                "frame index {index} out of bounds (static image has 1 frame)"
            )))
        }
    }
}
