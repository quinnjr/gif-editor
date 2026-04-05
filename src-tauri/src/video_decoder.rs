// Video decoder — extracts frames from MP4/WebM via ffmpeg/ffprobe.
//
// Uses ffprobe to read stream metadata (frame count, dimensions, frame rate)
// and ffmpeg to decode individual frames on demand.  An LRU cache bounds
// memory usage, matching the strategy used by GifData for GIF files.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::RgbaImage;
use lru::LruCache;

use crate::error::AppError;
use crate::frame_source::FrameSource;

const DEFAULT_CACHE_CAP: usize = 50;

pub struct VideoData {
    source_path: PathBuf,
    frame_count: usize,
    dimensions: (u32, u32),
    /// Frame delays in centiseconds (1/100 s), derived from the video frame
    /// rate so existing GIF-centric export code works unchanged.
    delays: Vec<u16>,
    fps: f64,
    frame_cache: LruCache<usize, RgbaImage>,
}

impl VideoData {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        Self::open_with_cache_cap(path, DEFAULT_CACHE_CAP)
    }

    pub fn open_with_cache_cap(path: &Path, cache_cap: usize) -> Result<Self, AppError> {
        if !path.exists() {
            return Err(AppError::VideoDecode(format!(
                "file not found: {}",
                path.display()
            )));
        }

        // Probe stream metadata with ffprobe.
        let probe_output = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                "-select_streams",
                "v:0",
            ])
            .arg(path)
            .output()
            .map_err(|e| AppError::VideoDecode(format!("failed to run ffprobe: {e}")))?;

        if !probe_output.status.success() {
            let stderr = String::from_utf8_lossy(&probe_output.stderr);
            return Err(AppError::VideoDecode(format!("ffprobe failed: {stderr}")));
        }

        let probe_json: serde_json::Value = serde_json::from_slice(&probe_output.stdout)
            .map_err(|e| AppError::VideoDecode(format!("failed to parse ffprobe output: {e}")))?;

        let stream = probe_json["streams"]
            .as_array()
            .and_then(|s| s.first())
            .ok_or_else(|| AppError::VideoDecode("no video stream found".to_string()))?;

        let width = stream["width"]
            .as_u64()
            .ok_or_else(|| AppError::VideoDecode("missing width".to_string()))?
            as u32;
        let height = stream["height"]
            .as_u64()
            .ok_or_else(|| AppError::VideoDecode("missing height".to_string()))?
            as u32;

        // Parse frame rate from r_frame_rate (e.g. "30/1" or "24000/1001").
        let fps = parse_frame_rate(stream["r_frame_rate"].as_str().unwrap_or("25/1"));

        // Count frames.  nb_frames is the most reliable when present, but
        // many containers don't set it.  Fall back to duration * fps.
        let frame_count = stream["nb_frames"]
            .as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .or_else(|| {
                let duration = stream["duration"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| {
                        probe_json["format"]["duration"]
                            .as_str()
                            .and_then(|s| s.parse::<f64>().ok())
                    })?;
                Some((duration * fps).round() as usize)
            })
            .unwrap_or(0);

        if frame_count == 0 {
            return Err(AppError::VideoDecode(
                "could not determine frame count".to_string(),
            ));
        }

        // Build per-frame delays in centiseconds, uniform for video.
        let delay_cs = (100.0 / fps).round() as u16;
        let delays = vec![delay_cs.max(1); frame_count];

        let cap = NonZeroUsize::new(cache_cap.max(1)).unwrap();

        Ok(Self {
            source_path: path.to_path_buf(),
            frame_count,
            dimensions: (width, height),
            delays,
            fps,
            frame_cache: LruCache::new(cap),
        })
    }

    /// Decode a single frame by seeking ffmpeg to the target timestamp and
    /// reading raw RGBA from stdout.
    fn decode_frame(&self, index: usize) -> Result<RgbaImage, AppError> {
        let (width, height) = self.dimensions;
        let timestamp = index as f64 / self.fps;

        let output = Command::new("ffmpeg")
            .args(["-ss", &format!("{timestamp:.6}"), "-i"])
            .arg(&self.source_path)
            .args([
                "-vframes", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-v", "quiet", "pipe:1",
            ])
            .output()
            .map_err(|e| AppError::VideoDecode(format!("failed to run ffmpeg: {e}")))?;

        if !output.status.success() {
            return Err(AppError::VideoDecode(format!(
                "ffmpeg frame extraction failed for frame {index}"
            )));
        }

        let expected_len = (width as usize) * (height as usize) * 4;
        if output.stdout.len() != expected_len {
            return Err(AppError::VideoDecode(format!(
                "unexpected frame size: got {} bytes, expected {expected_len}",
                output.stdout.len()
            )));
        }

        RgbaImage::from_raw(width, height, output.stdout).ok_or_else(|| {
            AppError::VideoDecode("failed to construct image from raw pixels".to_string())
        })
    }
}

impl FrameSource for VideoData {
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
        if index >= self.frame_count {
            return Err(AppError::VideoDecode(format!(
                "frame index {index} out of bounds (frame_count={})",
                self.frame_count
            )));
        }

        if let Some(cached) = self.frame_cache.get(&index) {
            return Ok(cached.clone());
        }

        let img = self.decode_frame(index)?;
        self.frame_cache.put(index, img.clone());
        Ok(img)
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Parse an ffprobe rational frame rate like "30/1" or "24000/1001" into an
/// f64.  Falls back to 25.0 if parsing fails.
fn parse_frame_rate(s: &str) -> f64 {
    if let Some((num, den)) = s.split_once('/') {
        let n: f64 = num.parse().unwrap_or(25.0);
        let d: f64 = den.parse().unwrap_or(1.0);
        if d > 0.0 { n / d } else { 25.0 }
    } else {
        s.parse().unwrap_or(25.0)
    }
}
