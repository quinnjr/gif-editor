use std::fs::File;
use std::path::{Path, PathBuf};

use gif::DecodeOptions;
use image::RgbaImage;
use lru::LruCache;
use std::num::NonZeroUsize;

use crate::error::AppError;
use crate::frame_source::FrameSource;

const DEFAULT_CACHE_CAP: usize = 50;

pub struct GifData {
    source_path: PathBuf,
    frame_count: usize,
    dimensions: (u32, u32),
    delays: Vec<u16>,
    frame_cache: LruCache<usize, RgbaImage>,
}

impl GifData {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        Self::open_with_cache_cap(path, DEFAULT_CACHE_CAP)
    }

    pub fn open_with_cache_cap(path: &Path, cache_cap: usize) -> Result<Self, AppError> {
        let file = File::open(path)?;
        let mut opts = DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = opts
            .read_info(file)
            .map_err(|e| AppError::GifDecode(e.to_string()))?;

        let width = decoder.width() as u32;
        let height = decoder.height() as u32;

        let mut delays = Vec::new();
        while let Some(frame) = decoder
            .read_next_frame()
            .map_err(|e| AppError::GifDecode(e.to_string()))?
        {
            delays.push(frame.delay);
        }

        let frame_count = delays.len();
        let cap = NonZeroUsize::new(cache_cap.max(1)).unwrap();

        Ok(Self {
            source_path: path.to_path_buf(),
            frame_count,
            dimensions: (width, height),
            delays,
            frame_cache: LruCache::new(cap),
        })
    }

    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn delays(&self) -> &[u16] {
        &self.delays
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn get_frame(&mut self, index: usize) -> Result<RgbaImage, AppError> {
        if index >= self.frame_count {
            return Err(AppError::GifDecode(format!(
                "frame index {} out of bounds (frame_count={})",
                index, self.frame_count
            )));
        }

        if let Some(cached) = self.frame_cache.get(&index) {
            return Ok(cached.clone());
        }

        let img = self.decode_frame(index)?;
        self.frame_cache.put(index, img.clone());
        Ok(img)
    }

    fn decode_frame(&self, index: usize) -> Result<RgbaImage, AppError> {
        let file = File::open(&self.source_path)?;
        let mut opts = DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = opts
            .read_info(file)
            .map_err(|e| AppError::GifDecode(e.to_string()))?;

        let mut current = 0usize;
        while let Some(frame) = decoder
            .read_next_frame()
            .map_err(|e| AppError::GifDecode(e.to_string()))?
        {
            if current == index {
                let (width, height) = self.dimensions;
                // frame.buffer contains RGBA pixels; it may be a sub-rectangle
                // positioned at (frame.left, frame.top). Build a full-canvas image.
                let mut canvas = RgbaImage::new(width, height);

                let fw = frame.width as u32;
                let fh = frame.height as u32;
                let fx = frame.left as u32;
                let fy = frame.top as u32;

                // The buffer length should be fw * fh * 4 bytes for RGBA output.
                let buf = frame.buffer.as_ref();
                for row in 0..fh {
                    for col in 0..fw {
                        let src_idx = ((row * fw + col) * 4) as usize;
                        let cx = fx + col;
                        let cy = fy + row;
                        if cx < width && cy < height && src_idx + 3 < buf.len() {
                            canvas.put_pixel(
                                cx,
                                cy,
                                image::Rgba([
                                    buf[src_idx],
                                    buf[src_idx + 1],
                                    buf[src_idx + 2],
                                    buf[src_idx + 3],
                                ]),
                            );
                        }
                    }
                }
                return Ok(canvas);
            }
            current += 1;
        }

        Err(AppError::GifDecode(format!(
            "could not decode frame {} (only {} frames found)",
            index, current
        )))
    }
}

impl FrameSource for GifData {
    fn frame_count(&self) -> usize {
        self.frame_count
    }

    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    fn delays(&self) -> &[u16] {
        &self.delays
    }

    fn source_path(&self) -> &Path {
        &self.source_path
    }

    fn get_frame(&mut self, index: usize) -> Result<RgbaImage, AppError> {
        // Delegate to the inherent method which handles caching.
        GifData::get_frame(self, index)
    }
}
