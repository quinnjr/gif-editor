// Video decoder — extracts frames from MP4/WebM via ffmpeg/ffprobe.
//
// Uses ffprobe to read stream metadata (frame count, dimensions, frame rate)
// and ffmpeg to decode frames.  Interactive access (`get_frame`) seeks and
// decodes one frame per ffmpeg invocation, with an LRU cache bounding memory
// usage.  Unlike GifData — which keeps a persistent decoder and is O(1) per
// sequential frame — each uncached `get_frame` spawns a subprocess, so bulk
// sequential consumers (e.g. export) should use `stream_frames`, which decodes
// every frame from a single ffmpeg invocation.

use std::io::{BufReader, Read};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use image::RgbaImage;
use lru::LruCache;

use crate::error::AppError;
use crate::frame_source::FrameSource;

const DEFAULT_CACHE_CAP: usize = 50;

/// Largest width or height accepted from probed metadata.
const MAX_DIMENSION: u32 = 16384;

/// Largest single decoded RGBA frame buffer (width * height * 4) accepted.
const MAX_FRAME_BYTES: usize = 512 * 1024 * 1024;

/// Largest frame count accepted from probed metadata.
const MAX_FRAME_COUNT: usize = 1_000_000;

/// Timeout for the ffprobe metadata probe when opening a video.
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);

/// Timeout for decoding a single frame — both the per-frame `get_frame`
/// subprocess and each frame read in `stream_frames`.
const FRAME_TIMEOUT: Duration = Duration::from_secs(30);

/// Run `cmd` to completion with a bounded wait, capturing stdout and stderr.
///
/// The child's stdin is closed and stdout/stderr are piped and drained on
/// background threads (so a chatty child can never deadlock on a full pipe).
/// The main thread polls `try_wait` until the child exits or `timeout`
/// elapses; on expiry the child is killed and reaped and an error string
/// `"<tool> timed out after <N>s"` is returned.  Callers map the error into
/// their own `AppError` variant.
pub(crate) fn run_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
    tool: &str,
) -> Result<Output, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn {tool}: {e}"))?;

    // Drain both pipes on background threads so the child can never block
    // on a full pipe while we poll for its exit.
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let stderr_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Killing closed the pipes, so the drain threads exit.
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err(format!("{tool} timed out after {}s", timeout.as_secs()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err(format!("failed to wait for {tool}: {e}"));
            }
        }
    };

    let stdout = stdout_thread.join().unwrap_or_default();
    let stderr = stderr_thread.join().unwrap_or_default();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

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

        // Probe stream metadata with ffprobe, with a bounded wait so a
        // wedged ffprobe cannot hang the app.
        let probe_output = run_with_timeout(
            Command::new("ffprobe")
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
                .arg(path),
            PROBE_TIMEOUT,
            "ffprobe",
        )
        .map_err(AppError::VideoDecode)?;

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

        validate_metadata(width, height, frame_count)?;

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

        let output = run_with_timeout(
            Command::new("ffmpeg")
                .args(["-ss", &format!("{timestamp:.6}"), "-i"])
                .arg(&self.source_path)
                .args([
                    "-vframes", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-v", "quiet", "pipe:1",
                ]),
            FRAME_TIMEOUT,
            "ffmpeg",
        )
        .map_err(AppError::VideoDecode)?;

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

    /// Decode source frames `[0, up_to)` sequentially from a single ffmpeg
    /// invocation, calling `on_frame(index, frame)` for each.
    ///
    /// This avoids the per-frame process spawn and seek cost of `get_frame`,
    /// making it the preferred path for whole-timeline operations like
    /// export.  `up_to` is clamped to the source frame count.  If the
    /// callback returns an error, decoding stops, the ffmpeg child is
    /// reaped, and the error is propagated.
    ///
    /// Returns an error if the stream ends before `up_to` frames arrive, or
    /// if ffmpeg produces no frame for `FRAME_TIMEOUT` (the deadline resets
    /// for every frame, so long videos are fine as long as ffmpeg keeps
    /// making progress).
    pub fn stream_frames(
        &self,
        up_to: usize,
        mut on_frame: impl FnMut(usize, RgbaImage) -> Result<(), AppError>,
    ) -> Result<(), AppError> {
        let up_to = up_to.min(self.frame_count);
        if up_to == 0 {
            return Ok(());
        }

        let (width, height) = self.dimensions;
        let frame_len = (width as usize) * (height as usize) * 4;

        let mut child = Command::new("ffmpeg")
            .arg("-i")
            .arg(&self.source_path)
            .args([
                "-f", "rawvideo", "-pix_fmt", "rgba", "-v", "error", "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::VideoDecode(format!("failed to spawn ffmpeg: {e}")))?;

        // Drain stderr on a background thread so ffmpeg can never block on a
        // full stderr pipe, and so decode failures can carry ffmpeg's own
        // diagnostics instead of an opaque "stream ended early".
        let mut stderr = child.stderr.take().expect("stderr was piped");
        let stderr_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf);
            buf
        });

        // A reader thread pulls raw frames off the pipe and hands them to
        // this thread over a bounded channel, letting us enforce a per-frame
        // deadline with `recv_timeout` instead of blocking forever in
        // `read_exact` if ffmpeg wedges.  The channel bound of 1 preserves
        // pipe-like backpressure so at most a couple of frames are buffered.
        let stdout = child.stdout.take().expect("stdout was piped");
        let (tx, rx) = mpsc::sync_channel::<std::io::Result<Vec<u8>>>(1);
        let reader = std::thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            let mut buf = vec![0u8; frame_len];
            for _ in 0..up_to {
                match stdout.read_exact(&mut buf) {
                    Ok(()) => {
                        // Send fails only when the receiver was dropped
                        // (early abort); just stop reading.
                        if tx.send(Ok(buf.clone())).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
            }
        });

        // Errors raised while pulling frames off the pipe are ffmpeg's fault
        // and get its stderr appended below; errors from the caller's
        // callback are propagated verbatim.
        enum StreamAbort {
            Ffmpeg(String),
            Other(AppError),
        }

        let result: Result<(), StreamAbort> = (|| {
            for index in 0..up_to {
                let bytes = match rx.recv_timeout(FRAME_TIMEOUT) {
                    Ok(Ok(bytes)) => bytes,
                    Ok(Err(e)) => {
                        return Err(StreamAbort::Ffmpeg(format!(
                            "ffmpeg stream ended early at frame {index} \
                             (expected {up_to} frames): {e}"
                        )));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        return Err(StreamAbort::Ffmpeg(format!(
                            "ffmpeg timed out after {}s while decoding frame {index}",
                            FRAME_TIMEOUT.as_secs()
                        )));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(StreamAbort::Ffmpeg(format!(
                            "ffmpeg stream ended early at frame {index} \
                             (expected {up_to} frames)"
                        )));
                    }
                };
                let img = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
                    StreamAbort::Other(AppError::VideoDecode(
                        "failed to construct image from raw pixels".to_string(),
                    ))
                })?;
                on_frame(index, img).map_err(StreamAbort::Other)?;
            }
            Ok(())
        })();

        // ffmpeg may still be running (more frames to write, or an aborted
        // read); kill it rather than leaving a zombie or blocking on a full
        // pipe, then reap it.  Killing closes the pipes, so both the reader
        // and stderr drain threads exit, and dropping `rx` unblocks a reader
        // stuck in `send`, so the joins cannot hang.
        let _ = child.kill();
        let _ = child.wait();
        drop(rx);
        let _ = reader.join();
        let stderr_buf = stderr_thread.join().unwrap_or_default();

        result.map_err(|abort| match abort {
            StreamAbort::Other(e) => e,
            StreamAbort::Ffmpeg(msg) => AppError::VideoDecode(match stderr_snippet(&stderr_buf) {
                Some(snippet) => format!("{msg}; ffmpeg stderr: {snippet}"),
                None => msg,
            }),
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

/// Validate probed stream metadata before it is used to size allocations.
///
/// ffprobe output is attacker-controlled (a crafted container can declare
/// arbitrary dimensions and frame counts), and downstream code allocates
/// `width * height * 4` frame buffers and a `frame_count`-sized delay vec.
/// Reject anything implausible up front with a clear error instead of
/// attempting a multi-gigabyte allocation.
fn validate_metadata(width: u32, height: u32, frame_count: usize) -> Result<(), AppError> {
    if width == 0 || height == 0 {
        return Err(AppError::VideoDecode(format!(
            "invalid video dimensions {width}x{height}"
        )));
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::VideoDecode(format!(
            "video dimensions {width}x{height} exceed the supported maximum \
             of {MAX_DIMENSION}x{MAX_DIMENSION}"
        )));
    }
    let frame_bytes = (width as usize) * (height as usize) * 4;
    if frame_bytes > MAX_FRAME_BYTES {
        return Err(AppError::VideoDecode(format!(
            "video frame buffer of {frame_bytes} bytes ({width}x{height} RGBA) \
             exceeds the supported maximum of {MAX_FRAME_BYTES} bytes"
        )));
    }
    if frame_count == 0 {
        return Err(AppError::VideoDecode(
            "could not determine frame count".to_string(),
        ));
    }
    if frame_count > MAX_FRAME_COUNT {
        return Err(AppError::VideoDecode(format!(
            "video frame count {frame_count} exceeds the supported maximum \
             of {MAX_FRAME_COUNT}"
        )));
    }
    Ok(())
}

/// Last ~2KB of captured ffmpeg stderr as trimmed lossy UTF-8, or `None`
/// when nothing usable was captured.
fn stderr_snippet(buf: &[u8]) -> Option<String> {
    const SNIPPET_LEN: usize = 2048;
    let tail = &buf[buf.len().saturating_sub(SNIPPET_LEN)..];
    let text = String::from_utf8_lossy(tail);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn err_msg(result: Result<(), AppError>) -> String {
        match result {
            Err(AppError::VideoDecode(msg)) => msg,
            other => panic!("expected VideoDecode error, got {other:?}"),
        }
    }

    #[test]
    fn validate_metadata_accepts_plausible_video() {
        assert!(validate_metadata(1920, 1080, 300).is_ok());
        // Boundary values are accepted: max dimension on one axis (with the
        // other small enough to keep the frame buffer in bounds) and the max
        // frame count.
        assert!(validate_metadata(MAX_DIMENSION, 1, MAX_FRAME_COUNT).is_ok());
        assert!(validate_metadata(1, MAX_DIMENSION, 1).is_ok());
    }

    #[test]
    fn validate_metadata_rejects_zero_dimensions() {
        let msg = err_msg(validate_metadata(0, 1080, 10));
        assert!(msg.contains("invalid video dimensions 0x1080"), "{msg}");
        let msg = err_msg(validate_metadata(1920, 0, 10));
        assert!(msg.contains("invalid video dimensions 1920x0"), "{msg}");
    }

    #[test]
    fn validate_metadata_rejects_oversized_dimensions() {
        let msg = err_msg(validate_metadata(MAX_DIMENSION + 1, 1080, 10));
        assert!(msg.contains("exceed the supported maximum"), "{msg}");
        let msg = err_msg(validate_metadata(1920, u32::MAX, 10));
        assert!(msg.contains("exceed the supported maximum"), "{msg}");
    }

    #[test]
    fn validate_metadata_rejects_oversized_frame_buffer() {
        // 12000x12000 passes the per-axis dimension check but the RGBA
        // buffer (576 MB) exceeds MAX_FRAME_BYTES.
        let msg = err_msg(validate_metadata(12000, 12000, 10));
        assert!(msg.contains("frame buffer"), "{msg}");
        assert!(msg.contains("exceeds the supported maximum"), "{msg}");
    }

    #[test]
    fn validate_metadata_rejects_bad_frame_counts() {
        let msg = err_msg(validate_metadata(64, 64, 0));
        assert!(msg.contains("could not determine frame count"), "{msg}");
        let msg = err_msg(validate_metadata(64, 64, MAX_FRAME_COUNT + 1));
        assert!(msg.contains("frame count"), "{msg}");
        assert!(msg.contains("exceeds the supported maximum"), "{msg}");
    }

    #[test]
    fn stderr_snippet_trims_and_truncates() {
        assert_eq!(stderr_snippet(b""), None);
        assert_eq!(stderr_snippet(b"  \n\t "), None);
        assert_eq!(stderr_snippet(b"  boom\n"), Some("boom".to_string()));
        // Only the trailing ~2KB survives.
        let mut long = vec![b'a'; 5000];
        long.extend_from_slice(b"tail-marker");
        let snippet = stderr_snippet(&long).unwrap();
        assert!(snippet.len() <= 2048);
        assert!(snippet.ends_with("tail-marker"));
    }

    #[cfg(unix)]
    #[test]
    fn run_with_timeout_kills_hung_process() {
        let start = Instant::now();
        let err = run_with_timeout(
            Command::new("sleep").arg("60"),
            Duration::from_millis(200),
            "sleep",
        )
        .unwrap_err();
        let elapsed = start.elapsed();
        assert!(err.contains("timed out"), "unexpected error: {err}");
        // Well under the 60s sleep proves the child was killed, not waited.
        assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
    }

    #[cfg(unix)]
    #[test]
    fn run_with_timeout_returns_output_for_fast_command() {
        let output = run_with_timeout(
            Command::new("sh").args(["-c", "printf hi"]),
            Duration::from_secs(10),
            "sh",
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"hi");
    }
}
