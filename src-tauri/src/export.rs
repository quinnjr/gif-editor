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

use image::imageops;
use serde::{Deserialize, Serialize};

use crate::compositor;
use crate::error::AppError;
use crate::frame_source::FrameSource;
use crate::layer::Layer;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Gif,
    Mp4,
    WebM,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSettings {
    pub format: ExportFormat,
    /// Perceptual quality, 0 = worst, 100 = best.
    pub quality: u8,
    /// Optional resize target (width, height) in pixels.
    pub resize: Option<(u32, u32)>,
}

// ---------------------------------------------------------------------------
// GIF export
// ---------------------------------------------------------------------------

/// Export `gif` composited with `layers` to a GIF file at `output_path`.
///
/// `on_progress` is called after each frame is written with the number of
/// frames completed so far (1-based).
pub fn export_gif(
    gif: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
    let frame_count = gif.frame_count();
    let (src_w, src_h) = gif.dimensions();
    let (out_w, out_h) = settings
        .resize
        .unwrap_or((src_w, src_h));

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

    for i in 0..frame_count {
        let base = gif.get_frame(i)?;
        let composited = compositor::composite_frame(&base, layers, i);

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
            .new_image(
                pixels.as_slice(),
                out_w as usize,
                out_h as usize,
                0.0,
            )
            .map_err(|e| AppError::Export(e.to_string()))?;

        let mut res = iq
            .quantize(&mut iq_image)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let (palette, indices) = res
            .remapped(&mut iq_image)
            .map_err(|e| AppError::Export(e.to_string()))?;

        // Build the flat palette bytes [R,G,B, ...] that gif expects.
        let palette_bytes: Vec<u8> = palette
            .iter()
            .flat_map(|c| [c.r, c.g, c.b])
            .collect();

        let delay = gif.delays()[i];
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

        on_progress(i + 1);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Video export (ffmpeg)
// ---------------------------------------------------------------------------

/// Export `gif` composited with `layers` to an MP4 or WebM file at
/// `output_path` by writing PNG frames to a temp directory and invoking
/// ffmpeg.
///
/// Returns `AppError::Export` if ffmpeg is not on PATH or exits non-zero.
pub fn export_video(
    gif: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
    if !ffmpeg_available() {
        return Err(AppError::Export(
            "ffmpeg not found on PATH; install ffmpeg to export video".to_string(),
        ));
    }

    let frame_count = gif.frame_count();
    let (src_w, src_h) = gif.dimensions();
    let (out_w, out_h) = settings.resize.unwrap_or((src_w, src_h));

    // Calculate average frame rate from delays (delay is in 1/100 s units).
    let avg_delay_cs: f64 = if frame_count == 0 {
        10.0
    } else {
        gif.delays().iter().map(|&d| d as f64).sum::<f64>() / frame_count as f64
    };
    // Clamp to avoid divide-by-zero; minimum 1 cs = 100 fps.
    let fps = 100.0 / avg_delay_cs.max(1.0);

    let temp_dir = tempfile::TempDir::new()?;

    for i in 0..frame_count {
        let base = gif.get_frame(i)?;
        let composited = compositor::composite_frame(&base, layers, i);

        let final_img = if (out_w, out_h) != (src_w, src_h) {
            imageops::resize(&composited, out_w, out_h, imageops::FilterType::Lanczos3)
        } else {
            composited
        };

        let png_path = temp_dir.path().join(format!("frame_{i:06}.png"));
        final_img
            .save(&png_path)
            .map_err(|e| AppError::Export(e.to_string()))?;

        on_progress(i + 1);
    }

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
        ExportFormat::Gif => {
            return Err(AppError::Export(
                "export_video called with Gif format; use export_gif instead".to_string(),
            ));
        }
    };

    let input_pattern = temp_dir
        .path()
        .join("frame_%06d.png")
        .to_string_lossy()
        .into_owned();

    let output_str = output_path.to_string_lossy().into_owned();
    let source_str = gif.source_path().to_string_lossy().into_owned();

    // Audio codec matched to container: AAC for MP4, Opus for WebM.
    let audio_codec = match settings.format {
        ExportFormat::Mp4 => "aac",
        ExportFormat::WebM => "libopus",
        ExportFormat::Gif => unreachable!(),
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-framerate",
        &format!("{fps:.3}"),
        "-i",
        &input_pattern,       // input 0: composited video frames
        "-i",
        &source_str,          // input 1: original file (for audio)
        "-map", "0:v",        // video from the PNG sequence
        "-map", "1:a?",       // audio from original (? = optional)
        "-c:v",
        codec,
        "-crf",
        &crf.to_string(),
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        audio_codec,
        "-shortest",          // stop when the shorter stream ends
        &output_str,
    ]);

    let status = cmd
        .status()
        .map_err(|e| AppError::Export(format!("failed to spawn ffmpeg: {e}")))?;

    if !status.success() {
        return Err(AppError::Export(format!(
            "ffmpeg exited with status {status}"
        )));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ffmpeg availability check
// ---------------------------------------------------------------------------

/// Return `true` if ffmpeg is available on PATH.
pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
