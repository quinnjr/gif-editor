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
    /// Virtual frame count — starts at 1 but expands when animated
    /// overlays (e.g. GIF layers) are added on top.
    frame_count: usize,
    /// Delay per virtual frame in centiseconds.
    delay: u16,
}

impl ImageSource {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let image = image::open(path)
            .map_err(|e| AppError::ImageLoad(e.to_string()))?
            .to_rgba8();
        Ok(Self {
            source_path: path.to_path_buf(),
            image,
            frame_count: 1,
            delay: 10, // 100ms per frame (10 centiseconds)
        })
    }
}

impl ImageSource {
    /// Expand the virtual timeline to `count` frames at `delay` cs each.
    /// Used when an animated GIF overlay is added on top of a static image.
    pub fn expand_timeline(&mut self, count: usize, delay: u16) {
        if count > self.frame_count {
            self.frame_count = count;
            self.delay = delay;
        }
    }
}

impl FrameSource for ImageSource {
    fn frame_count(&self) -> usize {
        self.frame_count
    }

    fn dimensions(&self) -> (u32, u32) {
        self.image.dimensions()
    }

    fn delays(&self) -> &[u16] {
        // Return a static slice for the common single-frame case;
        // callers that need per-frame delays use visible_delays()
        // which iterates 0..frame_count and indexes into this.
        // For expanded timelines this returns a 1-element slice —
        // all virtual frames share the same delay.
        std::slice::from_ref(&self.delay)
    }

    fn source_path(&self) -> &Path {
        &self.source_path
    }

    fn get_frame(&mut self, index: usize) -> Result<RgbaImage, AppError> {
        if index < self.frame_count {
            Ok(self.image.clone())
        } else {
            Err(AppError::ImageLoad(format!(
                "frame index {index} out of bounds (frame_count={})",
                self.frame_count
            )))
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
