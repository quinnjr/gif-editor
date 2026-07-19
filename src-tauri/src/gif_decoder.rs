use std::fs::File;
use std::path::{Path, PathBuf};

use gif::{DecodeOptions, Decoder, DisposalMethod};
use image::RgbaImage;
use lru::LruCache;
use std::num::NonZeroUsize;

use crate::error::AppError;
use crate::frame_source::FrameSource;

const DEFAULT_CACHE_CAP: usize = 50;

/// Disposal action recorded from the last decoded frame, applied to the
/// canvas *before* the next frame is drawn.
enum PendingDisposal {
    /// Keep/Any/None: leave the canvas as-is (next frame composites over it).
    Keep,
    /// Background: clear the previous frame's sub-rectangle to transparent.
    Background {
        left: u32,
        top: u32,
        width: u32,
        height: u32,
    },
    /// Previous: restore the canvas snapshot taken before the previous frame
    /// was drawn.
    Previous(RgbaImage),
}

/// Persistent sequential decoding state. GIF frames are delta-optimized, so
/// frame N can only be composited from the state after frame N-1. Keeping the
/// decoder and canvas alive between `get_frame` calls makes sequential
/// playback/export O(1) per frame instead of O(N).
struct DecodeState {
    decoder: Decoder<File>,
    /// Index of the frame the decoder will yield next.
    next_index: usize,
    /// Fully-composited canvas for frame `next_index - 1` (blank when
    /// `next_index == 0`).
    canvas: RgbaImage,
    /// Disposal of frame `next_index - 1`, to apply before drawing the next
    /// frame.
    pending: PendingDisposal,
}

pub struct GifData {
    source_path: PathBuf,
    frame_count: usize,
    dimensions: (u32, u32),
    delays: Vec<u16>,
    frame_cache: LruCache<usize, RgbaImage>,
    state: Option<DecodeState>,
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
            state: None,
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

    /// Opens a fresh decoder positioned before frame 0 with a blank canvas.
    fn fresh_state(&self) -> Result<DecodeState, AppError> {
        let file = File::open(&self.source_path)?;
        let mut opts = DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::RGBA);
        let decoder = opts
            .read_info(file)
            .map_err(|e| AppError::GifDecode(e.to_string()))?;
        let (width, height) = self.dimensions;
        Ok(DecodeState {
            decoder,
            next_index: 0,
            canvas: RgbaImage::new(width, height),
            pending: PendingDisposal::Keep,
        })
    }

    /// Decodes frame `index` with correct disposal-based accumulation.
    ///
    /// Resumes from the persistent sequential state when it has not yet
    /// passed `index`; otherwise restarts from frame 0. Every intermediate
    /// composited frame encountered on the way is added to the LRU cache.
    fn decode_frame(&mut self, index: usize) -> Result<RgbaImage, AppError> {
        let mut state = match self.state.take() {
            Some(s) if s.next_index <= index => s,
            _ => self.fresh_state()?,
        };

        while state.next_index <= index {
            let frame = state
                .decoder
                .read_next_frame()
                .map_err(|e| AppError::GifDecode(e.to_string()))?
                .ok_or_else(|| {
                    AppError::GifDecode(format!(
                        "could not decode frame {} (only {} frames found)",
                        index, state.next_index
                    ))
                })?;

            // 1. Apply the previous frame's disposal.
            match std::mem::replace(&mut state.pending, PendingDisposal::Keep) {
                PendingDisposal::Keep => {}
                PendingDisposal::Background {
                    left,
                    top,
                    width,
                    height,
                } => {
                    let (cw, ch) = self.dimensions;
                    for y in top..(top + height).min(ch) {
                        for x in left..(left + width).min(cw) {
                            state.canvas.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
                        }
                    }
                }
                PendingDisposal::Previous(snapshot) => {
                    state.canvas = snapshot;
                }
            }

            // 2. Snapshot the canvas if this frame must be reverted afterwards.
            let snapshot = if frame.dispose == DisposalMethod::Previous {
                Some(state.canvas.clone())
            } else {
                None
            };

            // 3. Composite the frame's sub-rectangle over the canvas. GIF
            // transparency is binary, so fully transparent source pixels
            // leave the underlying canvas untouched.
            let (cw, ch) = self.dimensions;
            let fw = frame.width as u32;
            let fh = frame.height as u32;
            let fx = frame.left as u32;
            let fy = frame.top as u32;
            let buf = frame.buffer.as_ref();
            for row in 0..fh {
                for col in 0..fw {
                    let src_idx = ((row * fw + col) * 4) as usize;
                    let cx = fx + col;
                    let cy = fy + row;
                    if cx < cw && cy < ch && src_idx + 3 < buf.len() && buf[src_idx + 3] != 0 {
                        state.canvas.put_pixel(
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

            // 4. Record this frame's disposal for the next iteration.
            state.pending = match frame.dispose {
                DisposalMethod::Background => PendingDisposal::Background {
                    left: fx,
                    top: fy,
                    width: fw,
                    height: fh,
                },
                DisposalMethod::Previous => PendingDisposal::Previous(snapshot.unwrap()),
                DisposalMethod::Any | DisposalMethod::Keep => PendingDisposal::Keep,
            };

            let decoded_index = state.next_index;
            state.next_index += 1;

            // Cache intermediate frames so nearby future requests hit the LRU.
            if decoded_index < index {
                self.frame_cache.put(decoded_index, state.canvas.clone());
            }
        }

        let result = state.canvas.clone();
        self.state = Some(state);
        Ok(result)
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

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
