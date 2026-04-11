// Project state — owns the open GIF, the layer stack, and a temp directory
// for decoded frame PNGs used by the compositor and the frontend preview.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::RgbaImage;
use serde::Serialize;
use uuid::Uuid;

use crate::compositor::composite_frame;
use crate::error::AppError;
use crate::frame_source::FrameSource;
use crate::gif_decoder::GifData;
use crate::image_source::ImageSource;
use crate::layer::{ImageLayer, Keyframe, Layer, Stroke, TextLayer};
use crate::video_decoder::VideoData;

// ---------------------------------------------------------------------------
// Serialisable types returned to / received from the frontend
// ---------------------------------------------------------------------------

/// Summary metadata sent to the frontend immediately after opening a GIF.
#[derive(Serialize, Clone)]
pub struct GifMetadata {
    pub frame_count: usize,
    pub width: u32,
    pub height: u32,
    pub delays: Vec<u16>,
}

/// A serialisable snapshot of a single layer for the frontend.
#[derive(Serialize, Clone)]
pub struct LayerInfo {
    pub id: Uuid,
    pub name: String,
    pub layer_type: String,
    pub position: (f64, f64),
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
    pub rotation: f64,
    pub opacity: f64,
    pub frame_range: (usize, usize),
    pub visible: bool,
    // Text-specific (None for image layers)
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<[u8; 4]>,
    pub stroke: Option<Stroke>,
    pub text_align: Option<String>,
    pub max_width: Option<f64>,
    // Image-specific (None for text layers)
    pub source_width: Option<u32>,
    pub source_height: Option<u32>,
    pub source_path: Option<String>,
    pub keyframes: Vec<Keyframe>,
}

impl From<&Layer> for LayerInfo {
    fn from(layer: &Layer) -> Self {
        match layer {
            Layer::Image(l) => LayerInfo {
                id: l.id,
                name: l.name.clone(),
                layer_type: "image".to_string(),
                position: l.position,
                scale_x: l.scale_x,
                scale_y: l.scale_y,
                skew_x: l.skew_x,
                skew_y: l.skew_y,
                rotation: l.rotation,
                opacity: l.opacity,
                frame_range: l.frame_range,
                visible: l.visible,
                text: None,
                font_family: None,
                font_size: None,
                color: None,
                stroke: None,
                text_align: None,
                max_width: None,
                source_width: Some(l.source_width),
                source_height: Some(l.source_height),
                source_path: l.source_path.clone(),
                keyframes: l.keyframes.clone(),
            },
            Layer::Text(l) => LayerInfo {
                id: l.id,
                name: l.name.clone(),
                layer_type: "text".to_string(),
                position: l.position,
                scale_x: l.scale_x,
                scale_y: l.scale_y,
                skew_x: l.skew_x,
                skew_y: l.skew_y,
                rotation: l.rotation,
                opacity: l.opacity,
                frame_range: l.frame_range,
                visible: l.visible,
                text: Some(l.text.clone()),
                font_family: Some(l.font_family.clone()),
                font_size: Some(l.font_size),
                color: Some(l.color),
                stroke: l.stroke.clone(),
                text_align: Some(l.text_align.clone()),
                max_width: l.max_width,
                source_width: None,
                source_height: None,
                source_path: None,
                keyframes: l.keyframes.clone(),
            },
        }
    }
}

/// Partial update payload received from the frontend.  Every field is
/// optional; only `Some` fields are applied.
#[derive(serde::Deserialize, Default)]
pub struct LayerUpdate {
    pub name: Option<String>,
    pub position: Option<(f64, f64)>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub skew_x: Option<f64>,
    pub skew_y: Option<f64>,
    pub rotation: Option<f64>,
    pub opacity: Option<f64>,
    pub frame_range: Option<(usize, usize)>,
    pub visible: Option<bool>,
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<[u8; 4]>,
    pub stroke: Option<Stroke>,
    pub text_align: Option<String>,
    pub max_width: Option<f64>,
    pub keyframes: Option<Vec<Keyframe>>,
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------

pub struct Project {
    pub source: Box<dyn FrameSource>,
    pub layers: Vec<Layer>,
    pub temp_dir: tempfile::TempDir,
    pub excluded_frames: BTreeSet<usize>,
}

/// A snapshot of mutable project state for undo/redo.
#[derive(Clone)]
pub struct HistoryEntry {
    pub layers: Vec<Layer>,
    pub excluded_frames: BTreeSet<usize>,
}

/// Container holding the optional open project plus undo/redo history stacks.
pub struct AppState {
    pub project: Option<Project>,
    pub history: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project: None,
            history: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Push a snapshot of the current project state onto the history stack.
/// Clears the redo stack. No-op if no project is open.
/// Cap: 50 entries (oldest dropped when full).
pub fn push_history(app_state: &mut AppState) {
    let Some(project) = &app_state.project else {
        return;
    };
    let entry = HistoryEntry {
        layers: project.layers.clone(),
        excluded_frames: project.excluded_frames.clone(),
    };
    if app_state.history.len() >= 50 {
        app_state.history.remove(0);
    }
    app_state.history.push(entry);
    app_state.redo_stack.clear();
}

/// Global app state: at most one project open at a time, with undo/redo history.
pub type ProjectState = Mutex<AppState>;

impl Project {
    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Open a media file at `path`, decode its metadata, and set up a temp
    /// directory for frame PNGs.  Supports GIF, MP4, and WebM.  Returns both
    /// the `Project` and the metadata struct for the frontend.
    pub fn open(path: &Path) -> Result<(Self, GifMetadata), AppError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let source: Box<dyn FrameSource> = match ext.as_str() {
            "gif" => Box::new(GifData::open(path)?),
            "mp4" | "webm" => Box::new(VideoData::open(path)?),
            "png" | "jpg" | "jpeg" | "webp" => Box::new(ImageSource::open(path)?),
            other => {
                return Err(AppError::VideoDecode(format!(
                    "unsupported file format: .{other}"
                )));
            }
        };

        let temp_dir = tempfile::TempDir::new()?;
        let project = Project {
            source,
            layers: Vec::new(),
            temp_dir,
            excluded_frames: BTreeSet::new(),
        };
        let metadata = project.visible_metadata();
        Ok((project, metadata))
    }

    /// Downcast the source to `ImageSource` if it is one.
    fn source_as_image_mut(&mut self) -> Option<&mut ImageSource> {
        self.source.as_any_mut().downcast_mut::<ImageSource>()
    }

    // -----------------------------------------------------------------------
    // Frame index mapping
    // -----------------------------------------------------------------------

    pub fn visible_frame_count(&self) -> usize {
        self.source.frame_count() - self.excluded_frames.len()
    }

    pub fn visible_delays(&self) -> Vec<u16> {
        let all_delays = self.source.delays();
        (0..self.source.frame_count())
            .filter(|i| !self.excluded_frames.contains(i))
            .map(|i| {
                // ImageSource may return a 1-element delay slice for an
                // expanded timeline — all virtual frames share the same delay.
                all_delays[i % all_delays.len()]
            })
            .collect()
    }

    pub fn visible_metadata(&self) -> GifMetadata {
        let (width, height) = self.source.dimensions();
        GifMetadata {
            frame_count: self.visible_frame_count(),
            width,
            height,
            delays: self.visible_delays(),
        }
    }

    pub fn logical_to_source(&self, logical: usize) -> Option<usize> {
        let total = self.source.frame_count();
        let mut count = 0usize;
        for src in 0..total {
            if self.excluded_frames.contains(&src) {
                continue;
            }
            if count == logical {
                return Some(src);
            }
            count += 1;
        }
        None
    }

    pub fn source_to_logical(&self, source: usize) -> Option<usize> {
        if self.excluded_frames.contains(&source) {
            return None;
        }
        let logical = (0..source)
            .filter(|i| !self.excluded_frames.contains(i))
            .count();
        Some(logical)
    }

    // -----------------------------------------------------------------------
    // Frame deletion / restoration
    // -----------------------------------------------------------------------

    pub fn delete_frames(&mut self, logical_indices: &[usize]) -> Result<GifMetadata, AppError> {
        let mut source_indices: Vec<usize> = Vec::new();
        for &li in logical_indices {
            if let Some(si) = self.logical_to_source(li) {
                source_indices.push(si);
            }
        }

        let new_excluded_count = self.excluded_frames.len() + source_indices.len();
        if new_excluded_count >= self.source.frame_count() {
            return Err(AppError::FrameDeletion(
                "cannot delete all frames; at least one must remain".to_string(),
            ));
        }

        let layer_source_ranges: Vec<(usize, usize)> = self
            .layers
            .iter()
            .map(|l| {
                let (ls, le) = l.frame_range();
                let ss = self.logical_to_source(ls).unwrap_or(0);
                let se = self.logical_to_source(le).unwrap_or(0);
                (ss, se)
            })
            .collect();

        let layer_source_keyframes: Vec<Vec<Keyframe>> = self
            .layers
            .iter()
            .map(|l| {
                l.keyframes()
                    .iter()
                    .filter_map(|kf| {
                        self.logical_to_source(kf.frame).map(|src| Keyframe {
                            frame: src,
                            position: kf.position,
                            opacity: kf.opacity,
                        })
                    })
                    .collect()
            })
            .collect();

        for si in &source_indices {
            self.excluded_frames.insert(*si);
        }

        self.remap_layer_ranges(&layer_source_ranges);
        self.remap_layer_keyframes(&layer_source_keyframes);
        Ok(self.visible_metadata())
    }

    pub fn restore_frames(&mut self, source_indices: &[usize]) -> Result<GifMetadata, AppError> {
        // Capture each layer's source range, then expand it to include any
        // contiguous restored frames that were previously excluded at the
        // boundaries.  This allows a delete→restore round-trip to recover the
        // original layer extent.
        let restoring: std::collections::BTreeSet<usize> = source_indices.iter().copied().collect();

        let layer_source_ranges: Vec<(usize, usize)> = self
            .layers
            .iter()
            .map(|l| {
                let (ls, le) = l.frame_range();
                let mut ss = self.logical_to_source(ls).unwrap_or(0);
                let mut se = self.logical_to_source(le).unwrap_or(0);

                // Extend start backward through contiguously restored frames.
                loop {
                    if ss == 0 {
                        break;
                    }
                    let prev = ss - 1;
                    if restoring.contains(&prev) && self.excluded_frames.contains(&prev) {
                        ss = prev;
                    } else {
                        break;
                    }
                }

                // Extend end forward through contiguously restored frames.
                let total = self.source.frame_count();
                loop {
                    let next = se + 1;
                    if next >= total {
                        break;
                    }
                    if restoring.contains(&next) && self.excluded_frames.contains(&next) {
                        se = next;
                    } else {
                        break;
                    }
                }

                (ss, se)
            })
            .collect();

        let layer_source_keyframes: Vec<Vec<Keyframe>> = self
            .layers
            .iter()
            .map(|l| {
                l.keyframes()
                    .iter()
                    .filter_map(|kf| {
                        self.logical_to_source(kf.frame).map(|src| Keyframe {
                            frame: src,
                            position: kf.position,
                            opacity: kf.opacity,
                        })
                    })
                    .collect()
            })
            .collect();

        for si in source_indices {
            self.excluded_frames.remove(si);
        }

        self.remap_layer_ranges(&layer_source_ranges);
        self.remap_layer_keyframes(&layer_source_keyframes);
        Ok(self.visible_metadata())
    }

    pub fn get_excluded_frames(&self) -> Vec<usize> {
        self.excluded_frames.iter().copied().collect()
    }

    fn remap_layer_ranges(&mut self, source_ranges: &[(usize, usize)]) {
        let visible_count = self.visible_frame_count();

        // Compute all new ranges before mutating layers to satisfy the borrow
        // checker: the mapping helpers only borrow self immutably.
        let new_ranges: Vec<(usize, usize)> = source_ranges
            .iter()
            .map(|&(src_start, src_end)| {
                let new_start = self
                    .source_to_logical(src_start)
                    .or_else(|| self.find_nearest_logical(src_start, true));
                let new_end = self
                    .source_to_logical(src_end)
                    .or_else(|| self.find_nearest_logical(src_end, false));

                match (new_start, new_end) {
                    (Some(s), Some(e)) if s <= e => (s, e),
                    (Some(s), Some(_)) => (s, s),
                    _ => (0, visible_count.saturating_sub(1)),
                }
            })
            .collect();

        for (layer, (ns, ne)) in self.layers.iter_mut().zip(new_ranges) {
            match layer {
                Layer::Image(l) => l.frame_range = (ns, ne),
                Layer::Text(l) => l.frame_range = (ns, ne),
            }
        }
    }

    fn remap_layer_keyframes(&mut self, source_keyframes: &[Vec<Keyframe>]) {
        // Pre-compute all new keyframe vectors before mutating layers so that
        // source_to_logical's immutable borrow of self does not conflict with
        // the subsequent mutable iteration — same pattern as remap_layer_ranges.
        let new_keyframes: Vec<Vec<Keyframe>> = source_keyframes
            .iter()
            .map(|src_kfs| {
                src_kfs
                    .iter()
                    .filter_map(|kf| {
                        self.source_to_logical(kf.frame).map(|new_frame| Keyframe {
                            frame: new_frame,
                            position: kf.position,
                            opacity: kf.opacity,
                        })
                    })
                    .collect()
            })
            .collect();

        for (layer, new_kfs) in self.layers.iter_mut().zip(new_keyframes) {
            match layer {
                Layer::Image(l) => l.keyframes = new_kfs,
                Layer::Text(l) => l.keyframes = new_kfs,
            }
        }
    }

    fn find_nearest_logical(&self, source: usize, search_forward: bool) -> Option<usize> {
        let total = self.source.frame_count();
        if search_forward {
            for s in source..total {
                if let Some(l) = self.source_to_logical(s) {
                    return Some(l);
                }
            }
            for s in (0..source).rev() {
                if let Some(l) = self.source_to_logical(s) {
                    return Some(l);
                }
            }
        } else {
            for s in (0..=source).rev() {
                if let Some(l) = self.source_to_logical(s) {
                    return Some(l);
                }
            }
            for s in source..total {
                if let Some(l) = self.source_to_logical(s) {
                    return Some(l);
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Frame access
    // -----------------------------------------------------------------------

    /// Return the filesystem path to a PNG of frame `logical_index`.
    ///
    /// The PNG is created on first access and cached by file existence so
    /// subsequent calls are cheap.
    pub fn get_frame_png_path(&mut self, logical_index: usize) -> Result<String, AppError> {
        let src_index = self.logical_to_source(logical_index).ok_or_else(|| {
            AppError::FrameDeletion(format!(
                "logical frame {logical_index} out of bounds (visible={})",
                self.visible_frame_count()
            ))
        })?;
        let png_path: PathBuf = self
            .temp_dir
            .path()
            .join(format!("frame_{src_index:05}.png"));

        if !png_path.exists() {
            let frame: RgbaImage = self.source.get_frame(src_index)?;
            frame
                .save(&png_path)
                .map_err(|e| AppError::Export(e.to_string()))?;
        }

        Ok(png_path.to_string_lossy().into_owned())
    }

    // -----------------------------------------------------------------------
    // Layer management
    // -----------------------------------------------------------------------

    /// Load an image from `path` and create a new `ImageLayer` covering all
    /// frames.
    pub fn add_image_layer(
        &mut self,
        path: &str,
    ) -> Result<(LayerInfo, Option<GifMetadata>), AppError> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let file_name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());

        let mut metadata_changed = false;

        let mut layer = if ext == "gif" {
            // Decode all GIF frames for animated overlay.
            let mut gif = GifData::open(Path::new(path))?;
            let (w, h) = gif.dimensions();
            let gif_frame_count = gif.frame_count();
            let mut frames = Vec::with_capacity(gif_frame_count);
            for i in 0..gif_frame_count {
                frames.push(gif.get_frame(i)?);
            }

            // If the base source is a static image and the GIF has more
            // frames, expand the project timeline to fit the animation.
            if gif_frame_count > self.visible_frame_count() {
                let avg_delay = if gif.delays().is_empty() {
                    10
                } else {
                    (gif.delays().iter().map(|&d| d as u32).sum::<u32>()
                        / gif.delays().len() as u32) as u16
                };
                if let Some(img_src) = self.source_as_image_mut() {
                    img_src.expand_timeline(gif_frame_count, avg_delay);
                    metadata_changed = true;
                }
            }

            let mut l = ImageLayer::new(file_name, w, h);
            l.image_data = frames.first().cloned();
            l.frames = frames;
            l
        } else {
            // Static image.
            let img = image::open(path)
                .map_err(|e| AppError::ImageLoad(e.to_string()))?
                .to_rgba8();
            let (w, h) = img.dimensions();
            let mut l = ImageLayer::new(file_name, w, h);
            l.image_data = Some(img);
            l
        };

        let frame_count = self.visible_frame_count();
        layer.source_path = Some(path.to_string());
        layer.frame_range = (0, frame_count.saturating_sub(1));

        let info = LayerInfo::from(&Layer::Image(layer.clone()));
        self.layers.push(Layer::Image(layer));

        let new_meta = if metadata_changed {
            Some(self.visible_metadata())
        } else {
            None
        };

        Ok((info, new_meta))
    }

    /// Create a new `TextLayer` covering all frames.
    pub fn add_text_layer(
        &mut self,
        text: String,
        font_family: Option<String>,
        font_size: Option<f64>,
        color: Option<[u8; 4]>,
        stroke: Option<Stroke>,
    ) -> LayerInfo {
        let frame_count = self.visible_frame_count();
        let mut layer = TextLayer::new(text);
        if let Some(ff) = font_family {
            layer.font_family = ff;
        }
        if let Some(fs) = font_size {
            layer.font_size = fs;
        }
        if let Some(c) = color {
            layer.color = c;
        }
        if stroke.is_some() {
            layer.stroke = stroke;
        }
        layer.frame_range = (0, frame_count.saturating_sub(1));

        let info = LayerInfo::from(&Layer::Text(layer.clone()));
        self.layers.push(Layer::Text(layer));
        info
    }

    /// Apply a partial update to the layer identified by `id`.
    pub fn update_layer(&mut self, id: Uuid, changes: LayerUpdate) -> Result<LayerInfo, AppError> {
        let layer = self
            .layers
            .iter_mut()
            .find(|l| l.id() == id)
            .ok_or(AppError::LayerNotFound(id))?;

        match layer {
            Layer::Image(l) => {
                if let Some(v) = changes.name {
                    l.name = v;
                }
                if let Some(v) = changes.position {
                    l.position = v;
                }
                if let Some(v) = changes.scale_x {
                    l.scale_x = v;
                }
                if let Some(v) = changes.scale_y {
                    l.scale_y = v;
                }
                if let Some(v) = changes.skew_x {
                    l.skew_x = v;
                }
                if let Some(v) = changes.skew_y {
                    l.skew_y = v;
                }
                if let Some(v) = changes.rotation {
                    l.rotation = v;
                }
                if let Some(v) = changes.opacity {
                    l.opacity = v;
                }
                if let Some(v) = changes.frame_range {
                    l.frame_range = v;
                }
                if let Some(v) = changes.visible {
                    l.visible = v;
                }
                if let Some(v) = changes.keyframes {
                    l.keyframes = v;
                }
                // Text fields are silently ignored for image layers.
            }
            Layer::Text(l) => {
                if let Some(v) = changes.name {
                    l.name = v;
                }
                if let Some(v) = changes.position {
                    l.position = v;
                }
                if let Some(v) = changes.scale_x {
                    l.scale_x = v;
                }
                if let Some(v) = changes.scale_y {
                    l.scale_y = v;
                }
                if let Some(v) = changes.skew_x {
                    l.skew_x = v;
                }
                if let Some(v) = changes.skew_y {
                    l.skew_y = v;
                }
                if let Some(v) = changes.rotation {
                    l.rotation = v;
                }
                if let Some(v) = changes.opacity {
                    l.opacity = v;
                }
                if let Some(v) = changes.frame_range {
                    l.frame_range = v;
                }
                if let Some(v) = changes.visible {
                    l.visible = v;
                }
                if let Some(v) = changes.text {
                    l.text = v;
                }
                if let Some(v) = changes.font_family {
                    l.font_family = v;
                }
                if let Some(v) = changes.font_size {
                    l.font_size = v;
                }
                if let Some(v) = changes.color {
                    l.color = v;
                }
                if changes.stroke.is_some() {
                    l.stroke = changes.stroke;
                }
                if let Some(v) = changes.text_align {
                    l.text_align = v;
                }
                if changes.max_width.is_some() {
                    l.max_width = changes.max_width;
                }
                if let Some(v) = changes.keyframes {
                    l.keyframes = v;
                }
            }
        }

        Ok(LayerInfo::from(&*layer))
    }

    /// Remove the layer with the given `id`.
    pub fn remove_layer(&mut self, id: Uuid) -> Result<(), AppError> {
        let pos = self
            .layers
            .iter()
            .position(|l| l.id() == id)
            .ok_or(AppError::LayerNotFound(id))?;
        self.layers.remove(pos);
        Ok(())
    }

    /// Reorder `self.layers` to match the order specified by `ids`.
    ///
    /// Every id in `ids` must correspond to an existing layer; extra layers
    /// not present in `ids` are dropped (the frontend is expected to send the
    /// complete new order).
    pub fn reorder_layers(&mut self, ids: Vec<Uuid>) -> Result<(), AppError> {
        let mut reordered: Vec<Layer> = Vec::with_capacity(ids.len());
        for id in &ids {
            let pos = self
                .layers
                .iter()
                .position(|l| l.id() == *id)
                .ok_or(AppError::LayerNotFound(*id))?;
            reordered.push(self.layers[pos].clone());
        }
        self.layers = reordered;
        Ok(())
    }

    /// Flip a layer horizontally or vertically by negating the appropriate scale axis.
    pub fn flip_layer(&mut self, id: Uuid, axis: &str) -> Result<LayerInfo, AppError> {
        let layer = self
            .layers
            .iter_mut()
            .find(|l| l.id() == id)
            .ok_or(AppError::LayerNotFound(id))?;

        match layer {
            Layer::Image(l) => match axis {
                "horizontal" => l.scale_x *= -1.0,
                "vertical" => l.scale_y *= -1.0,
                _ => {}
            },
            Layer::Text(l) => match axis {
                "horizontal" => l.scale_x *= -1.0,
                "vertical" => l.scale_y *= -1.0,
                _ => {}
            },
        }

        Ok(LayerInfo::from(&*layer))
    }

    /// Clone the layer identified by `id` with a fresh UUID and insert it
    /// immediately after the source in the layer stack.
    pub fn duplicate_layer(&mut self, id: Uuid) -> Result<LayerInfo, AppError> {
        let pos = self
            .layers
            .iter()
            .position(|l| l.id() == id)
            .ok_or(AppError::LayerNotFound(id))?;

        let mut new_layer = self.layers[pos].clone();
        // Assign a fresh UUID to the duplicate.
        match &mut new_layer {
            Layer::Image(l) => l.id = uuid::Uuid::new_v4(),
            Layer::Text(l) => l.id = uuid::Uuid::new_v4(),
        }

        let info = LayerInfo::from(&new_layer);
        self.layers.insert(pos + 1, new_layer);
        Ok(info)
    }

    /// Composite all layers onto the GIF frame at `logical_index`, save the
    /// result as a PNG in the temp directory, and return its path.
    pub fn render_composite(&mut self, logical_index: usize) -> Result<String, AppError> {
        let src_index = self.logical_to_source(logical_index).ok_or_else(|| {
            AppError::FrameDeletion(format!(
                "logical frame {logical_index} out of bounds (visible={})",
                self.visible_frame_count()
            ))
        })?;
        let base: RgbaImage = self.source.get_frame(src_index)?;
        let composited = composite_frame(&base, &self.layers, logical_index);

        let out_path: PathBuf = self
            .temp_dir
            .path()
            .join(format!("composite_{src_index:05}.png"));
        composited
            .save(&out_path)
            .map_err(|e| AppError::Export(e.to_string()))?;

        Ok(out_path.to_string_lossy().into_owned())
    }

    /// Return a snapshot of all layers in current stack order.
    pub fn get_layers(&self) -> Vec<LayerInfo> {
        self.layers.iter().map(LayerInfo::from).collect()
    }
}
