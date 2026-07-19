// Export pipeline — GIF and video output.
//
// GIF export: composites every frame, quantises to a palette with imagequant,
// and encodes with the gif crate.  This avoids the "last palette wins" artefact
// that naive re-encoding produces.
//
// Video export: composites frames to a temporary directory of PNGs then shells
// out to ffmpeg, which must be on PATH.  libx264 is used for MP4 and
// libvpx-vp9 for WebM.  The quality 0–100 range is mapped to the codec's CRF
// scale linearly.

use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use image::imageops;
use serde::{Deserialize, Serialize};

use crate::compositor;
use crate::error::AppError;
use crate::frame_source::FrameSource;
use crate::layer::Layer;
use crate::video_decoder::{VideoData, run_with_timeout};

// ---------------------------------------------------------------------------
// Shared frame iteration
// ---------------------------------------------------------------------------

/// Drive `f(base, logical)` over the selected source frames in logical order.
///
/// Video sources decode a frame per ffmpeg subprocess in `get_frame`, so for
/// them every needed frame is streamed from ONE ffmpeg invocation instead,
/// skipping unselected source frames.  This requires `frame_indices` to be
/// strictly increasing, which holds for the exclusion-derived logical→source
/// map (order-preserving); fall back to per-frame `get_frame` otherwise, and
/// for all other sources.
fn for_each_selected_frame(
    source: &mut dyn FrameSource,
    frame_indices: &[usize],
    mut f: impl FnMut(&image::RgbaImage, usize) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let sequential = frame_indices.windows(2).all(|w| w[0] < w[1]);
    if sequential
        && !frame_indices.is_empty()
        && let Some(video) = source.as_any_mut().downcast_mut::<VideoData>()
    {
        let last = *frame_indices.last().unwrap();
        let mut next = 0usize; // next unwritten position in frame_indices
        video.stream_frames(last + 1, |src_i, base| {
            if next < frame_indices.len() && frame_indices[next] == src_i {
                f(&base, next)?;
                next += 1;
            }
            Ok(())
        })?;
        if next != frame_indices.len() {
            return Err(AppError::Export(format!(
                "video stream ended before all selected frames were decoded \
                 ({next} of {} written)",
                frame_indices.len()
            )));
        }
        return Ok(());
    }

    for (logical, &src_i) in frame_indices.iter().enumerate() {
        let base = source.get_frame(src_i)?;
        f(&base, logical)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Gif,
    Mp4,
    WebM,
    Png,
    Jpeg,
    WebP,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    pub format: ExportFormat,
    /// Perceptual quality, 0 = worst, 100 = best.
    ///
    /// Applies to GIF quantisation, video CRF (MP4/WebM), and JPEG encoding.
    /// WebP stills are always written lossless and ignore this field.
    pub quality: u8,
    /// Optional resize target (width, height) in pixels.
    pub resize: Option<(u32, u32)>,
    /// Frame index to export for still-image formats (Png, Jpeg, WebP).
    /// Ignored for animated formats (Gif, Mp4, WebM).
    pub frame_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Exporting placeholder + export dispatch
// ---------------------------------------------------------------------------

/// Placeholder frame source installed in the project while an export owns
/// the real source.
///
/// Metadata queries (`frame_count`, `dimensions`, `delays`, `source_path`)
/// report the cached values so timeline state stays sane, but fetching pixel
/// data fails with "export in progress" until the real source is restored.
pub struct ExportingPlaceholder {
    frame_count: usize,
    dimensions: (u32, u32),
    delays: Vec<u16>,
    source_path: std::path::PathBuf,
}

impl ExportingPlaceholder {
    /// Snapshot `source`'s metadata into a placeholder that can stand in for
    /// it while the real source is moved out for an export.
    pub fn for_source(source: &dyn FrameSource) -> Self {
        Self {
            frame_count: source.frame_count(),
            dimensions: source.dimensions(),
            delays: source.delays().to_vec(),
            source_path: source.source_path().to_path_buf(),
        }
    }
}

impl FrameSource for ExportingPlaceholder {
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

    fn get_frame(&mut self, _index: usize) -> Result<image::RgbaImage, AppError> {
        Err(AppError::Export("export in progress".into()))
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Everything an export needs, snapshotted from the project under the state
/// lock (see `Project::take_source_for_export`) so the export itself can run
/// without holding the lock.
pub struct ExportSnapshot {
    /// The real frame source, moved out of the project for the export's
    /// duration (an `ExportingPlaceholder` stands in for it meanwhile).
    pub source: Box<dyn FrameSource>,
    /// Layer stack clone (cheap: pixel buffers are Arc-shared).
    pub layers: Vec<Layer>,
    /// Source frame indices to export, in logical (timeline) order.
    pub frame_indices: Vec<usize>,
    /// Per-logical-frame delays in 1/100 s units.
    pub delays: Vec<u16>,
    /// `(source_index, logical_index)` for still-image formats, resolved
    /// while the project was intact; `None` for animated formats.
    pub still_frame: Option<(usize, usize)>,
}

impl std::fmt::Debug for ExportSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `source` is an opaque trait object; show its metadata instead.
        f.debug_struct("ExportSnapshot")
            .field("source_path", &self.source.source_path())
            .field("source_frame_count", &self.source.frame_count())
            .field("layers", &self.layers.len())
            .field("frame_indices", &self.frame_indices)
            .field("delays", &self.delays)
            .field("still_frame", &self.still_frame)
            .finish()
    }
}

/// Dispatch an export to the format-specific implementation.
///
/// On error the partial (possibly corrupt) output file is removed so a
/// failed export never leaves garbage at the user's chosen path.
pub fn run_export(
    snapshot: &mut ExportSnapshot,
    settings: &ExportSettings,
    output_path: &Path,
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
    let ExportSnapshot {
        source,
        layers,
        frame_indices,
        delays,
        still_frame,
    } = snapshot;

    let result = match settings.format {
        ExportFormat::Gif => export_gif(
            source.as_mut(),
            layers,
            settings,
            output_path,
            frame_indices,
            delays,
            on_progress,
        ),
        ExportFormat::Mp4 | ExportFormat::WebM => export_video(
            source.as_mut(),
            layers,
            settings,
            output_path,
            frame_indices,
            delays,
            on_progress,
        ),
        ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::WebP => match *still_frame {
            Some((src, logical)) => {
                export_image(source.as_mut(), layers, settings, output_path, src, logical)
            }
            None => Err(AppError::Export(
                "still-image export requires a resolved frame index".into(),
            )),
        },
    };

    if result.is_err() {
        // export_gif creates the file up front and export_video's ffmpeg
        // writes it directly, so a failure mid-export leaves a truncated or
        // corrupt file behind — remove it.
        let _ = std::fs::remove_file(output_path);
    }
    result
}

// ---------------------------------------------------------------------------
// GIF export
// ---------------------------------------------------------------------------

/// Export `source` composited with `layers` to a GIF file at `output_path`.
///
/// Only the frames identified by `frame_indices` (source frame indices) are
/// exported, in order, using the corresponding `delays` for each.
///
/// `on_progress` is called after each frame is written with the number of
/// frames completed so far (1-based).
pub fn export_gif(
    source: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    frame_indices: &[usize],
    delays: &[u16],
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
    let (src_w, src_h) = source.dimensions();
    let (out_w, out_h) = settings.resize.unwrap_or((src_w, src_h));

    // imagequant quality maps 0–100 directly.
    let iq_quality = settings.quality;

    let out_file = File::create(output_path)?;
    // gif::Encoder takes the global palette as a slice; we pass empty and let
    // each frame carry its own local palette instead.
    let mut encoder = gif::Encoder::new(out_file, out_w as u16, out_h as u16, &[])
        .map_err(|e| AppError::Export(e.to_string()))?;
    encoder
        .set_repeat(gif::Repeat::Infinite)
        .map_err(|e| AppError::Export(e.to_string()))?;

    // Composite, quantise, and encode one logical frame.
    let mut encode_frame = |base: &image::RgbaImage, logical: usize| -> Result<(), AppError> {
        let composited = compositor::composite_frame(base, layers, logical);

        // Resize if requested.
        let final_img = if (out_w, out_h) != (src_w, src_h) {
            imageops::resize(&composited, out_w, out_h, imageops::FilterType::Lanczos3)
        } else {
            composited
        };

        // Quantise RGBA to a 256-colour palette with imagequant.
        let mut iq = imagequant::new();
        iq.set_quality(0, iq_quality)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let pixels: Vec<imagequant::RGBA> = final_img
            .pixels()
            .map(|p| imagequant::RGBA {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            })
            .collect();

        let mut iq_image = iq
            .new_image(pixels.as_slice(), out_w as usize, out_h as usize, 0.0)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let mut res = iq
            .quantize(&mut iq_image)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let (palette, indices) = res
            .remapped(&mut iq_image)
            .map_err(|e| AppError::Export(e.to_string()))?;

        // Build the flat palette bytes [R,G,B, ...] that gif expects.
        let palette_bytes: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();

        let delay = delays[logical];
        let mut frame = gif::Frame::from_palette_pixels(
            out_w as u16,
            out_h as u16,
            &*indices,
            &*palette_bytes,
            None,
        );
        frame.delay = delay;

        encoder
            .write_frame(&frame)
            .map_err(|e| AppError::Export(e.to_string()))?;

        on_progress(logical + 1);
        Ok(())
    };

    for_each_selected_frame(source, frame_indices, &mut encode_frame)
}

// ---------------------------------------------------------------------------
// Video export (ffmpeg)
// ---------------------------------------------------------------------------

/// Export `source` composited with `layers` to an MP4 or WebM file at
/// `output_path` by writing PNG frames to a temp directory and invoking
/// ffmpeg.
///
/// Only the frames identified by `frame_indices` (source frame indices) are
/// exported, in order, using the corresponding `delays` for average fps
/// calculation.
///
/// Returns `AppError::Export` if ffmpeg is not on PATH or exits non-zero.
pub fn export_video(
    source: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    frame_indices: &[usize],
    delays: &[u16],
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
    // Validate the format up front, before the ffmpeg gate and before any
    // frame is composited or written: a caller bug should fail fast, not
    // after minutes of frame writing (and regardless of ffmpeg presence).
    //
    // Map quality 0–100 → CRF.
    // libx264: CRF 0 (lossless) – 51 (worst); quality 100 → CRF 0, quality 0 → CRF 51.
    // libvpx-vp9: CRF 0–63; quality 100 → CRF 0, quality 0 → CRF 63.
    let (codec, crf) = match settings.format {
        ExportFormat::Mp4 => {
            let crf = (51.0 * (1.0 - settings.quality as f64 / 100.0)).round() as u32;
            ("libx264", crf)
        }
        ExportFormat::WebM => {
            let crf = (63.0 * (1.0 - settings.quality as f64 / 100.0)).round() as u32;
            ("libvpx-vp9", crf)
        }
        ExportFormat::Gif | ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::WebP => {
            return Err(AppError::Export(
                "export_video called with non-video format; use export_gif or export_image instead"
                    .to_string(),
            ));
        }
    };

    if !ffmpeg_available() {
        return Err(AppError::Export(
            "ffmpeg not found on PATH; install ffmpeg to export video".to_string(),
        ));
    }

    let frame_count = frame_indices.len();
    let (src_w, src_h) = source.dimensions();
    let (out_w, out_h) = settings.resize.unwrap_or((src_w, src_h));

    // Calculate average frame rate from delays (delay is in 1/100 s units).
    let avg_delay_cs: f64 = if frame_count == 0 {
        10.0
    } else {
        delays.iter().map(|&d| d as f64).sum::<f64>() / frame_count as f64
    };
    // Clamp to avoid divide-by-zero; minimum 1 cs = 100 fps.
    let fps = 100.0 / avg_delay_cs.max(1.0);

    let temp_dir = tempfile::TempDir::new()?;

    // Composite, resize, and write one logical frame as a PNG.
    let mut write_frame = |base: &image::RgbaImage, logical: usize| -> Result<(), AppError> {
        let composited = compositor::composite_frame(base, layers, logical);

        let final_img = if (out_w, out_h) != (src_w, src_h) {
            imageops::resize(&composited, out_w, out_h, imageops::FilterType::Lanczos3)
        } else {
            composited
        };

        // These PNGs exist only for ffmpeg to immediately re-read, so trade
        // file size for encode speed (fast compression, no filtering) via the
        // shared temp-PNG helper.  The final still-image PNG export
        // (export_image) keeps default quality.
        let png_path = temp_dir.path().join(format!("frame_{logical:06}.png"));
        crate::project::save_temp_png(&final_img, &png_path)?;
        on_progress(logical + 1);
        Ok(())
    };

    for_each_selected_frame(source, frame_indices, &mut write_frame)?;

    let input_pattern = temp_dir
        .path()
        .join("frame_%06d.png")
        .to_string_lossy()
        .into_owned();

    let output_str = output_path.to_string_lossy().into_owned();
    let source_str = source.source_path().to_string_lossy().into_owned();

    // Audio codec matched to container: AAC for MP4, Opus for WebM.
    let audio_codec = match settings.format {
        ExportFormat::Mp4 => "aac",
        ExportFormat::WebM => "libopus",
        ExportFormat::Gif | ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::WebP => {
            unreachable!()
        }
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-framerate",
        &format!("{fps:.3}"),
        "-i",
        &input_pattern, // input 0: composited video frames
        "-i",
        &source_str, // input 1: original file (for audio)
        "-map",
        "0:v", // video from the PNG sequence
        "-map",
        "1:a?", // audio from original (? = optional)
        "-c:v",
        codec,
        "-crf",
        &crf.to_string(),
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        audio_codec,
        "-shortest", // stop when the shorter stream ends
        &output_str,
    ]);

    // The final encode is long-running by design, so instead of a fixed
    // timeout use a generous one derived from the work size: a 60 s floor
    // plus 2 s per frame.  Real encodes of small PNG frames finish far
    // faster; only a wedged ffmpeg gets anywhere near the deadline.
    let encode_timeout = Duration::from_secs(60 + 2 * frame_count as u64);
    let output = run_with_timeout(&mut cmd, encode_timeout, "ffmpeg").map_err(AppError::Export)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Export(format!(
            "ffmpeg exited with status {}: {}",
            output.status,
            stderr.trim()
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Still image export (PNG, JPEG, WebP)
// ---------------------------------------------------------------------------

/// Flatten an RGBA image onto a white background, producing an RGB image.
///
/// JPEG does not support transparency; this converts semi-transparent pixels
/// to their visually equivalent opaque colour against white.
fn flatten_alpha(img: &image::RgbaImage) -> image::RgbImage {
    let (w, h) = img.dimensions();
    let mut out = image::RgbImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels() {
        let a = pixel[3] as f32 / 255.0;
        let r = (a * pixel[0] as f32 + (1.0 - a) * 255.0).round() as u8;
        let g = (a * pixel[1] as f32 + (1.0 - a) * 255.0).round() as u8;
        let b = (a * pixel[2] as f32 + (1.0 - a) * 255.0).round() as u8;
        out.put_pixel(x, y, image::Rgb([r, g, b]));
    }
    out
}

/// Export a single composited frame as a PNG, JPEG, or WebP still image.
///
/// `source_index` is the source frame index to fetch from `source`;
/// `logical_index` is the corresponding logical (visible-timeline) index used
/// to composite layers, whose frame ranges and keyframes live in logical
/// space.  The two differ when frames have been deleted.
/// Returns `AppError::Export` if called with an animated format.
pub fn export_image(
    source: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    source_index: usize,
    logical_index: usize,
) -> Result<(), AppError> {
    let (src_w, src_h) = source.dimensions();
    let (out_w, out_h) = settings.resize.unwrap_or((src_w, src_h));

    let base = source.get_frame(source_index)?;
    let composited = compositor::composite_frame(&base, layers, logical_index);

    let final_img = if (out_w, out_h) != (src_w, src_h) {
        imageops::resize(&composited, out_w, out_h, imageops::FilterType::Lanczos3)
    } else {
        composited
    };

    match settings.format {
        ExportFormat::Png => {
            image::DynamicImage::ImageRgba8(final_img)
                .save(output_path)
                .map_err(|e| AppError::Export(e.to_string()))?;
        }
        ExportFormat::Jpeg => {
            let rgb = flatten_alpha(&final_img);
            let file = File::create(output_path)?;
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::BufWriter::new(file),
                settings.quality,
            );
            image::DynamicImage::ImageRgb8(rgb)
                .write_with_encoder(encoder)
                .map_err(|e| AppError::Export(e.to_string()))?;
        }
        ExportFormat::WebP => {
            // `settings.quality` is intentionally ignored here: WebP stills
            // are always written lossless (see the ExportSettings::quality
            // doc).
            image::DynamicImage::ImageRgba8(final_img)
                .save(output_path)
                .map_err(|e| AppError::Export(e.to_string()))?;
        }
        _ => {
            return Err(AppError::Export(
                "export_image called with animated format; use export_gif or export_video"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ffmpeg availability check
// ---------------------------------------------------------------------------

/// Return `true` if ffmpeg is available on PATH.
///
/// The probe (`ffmpeg -version`, bounded at 5 s) runs once per process and
/// the result is memoized for the rest of the session — the export dialog
/// calls this on every open, and PATH changes mid-session are not a
/// supported scenario.  A wedged or missing ffmpeg therefore costs at most
/// one bounded probe.
pub fn ffmpeg_available() -> bool {
    static FFMPEG_AVAILABLE: OnceLock<bool> = OnceLock::new();
    *FFMPEG_AVAILABLE.get_or_init(|| {
        run_with_timeout(
            Command::new("ffmpeg").arg("-version"),
            Duration::from_secs(5),
            "ffmpeg",
        )
        .map(|o| o.status.success())
        .unwrap_or(false)
    })
}
