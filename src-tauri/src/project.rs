// Project state — owns the open GIF, the layer stack, and a temp directory
// for decoded frame PNGs used by the compositor and the frontend preview.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::RgbaImage;
use serde::Serialize;
use uuid::Uuid;

use crate::compositor::composite_frame;
use crate::error::AppError;
use crate::gif_decoder::GifData;
use crate::layer::{ImageLayer, Layer, Stroke, TextLayer};

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
    pub scale: f64,
    pub opacity: f64,
    pub frame_range: (usize, usize),
    pub visible: bool,
    // Text-specific (None for image layers)
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<[u8; 4]>,
    pub stroke: Option<Stroke>,
    // Image-specific (None for text layers)
    pub source_width: Option<u32>,
    pub source_height: Option<u32>,
    pub source_path: Option<String>,
}

impl From<&Layer> for LayerInfo {
    fn from(layer: &Layer) -> Self {
        match layer {
            Layer::Image(l) => LayerInfo {
                id: l.id,
                name: l.name.clone(),
                layer_type: "image".to_string(),
                position: l.position,
                scale: l.scale,
                opacity: l.opacity,
                frame_range: l.frame_range,
                visible: l.visible,
                text: None,
                font_family: None,
                font_size: None,
                color: None,
                stroke: None,
                source_width: Some(l.source_width),
                source_height: Some(l.source_height),
                source_path: l.source_path.clone(),
            },
            Layer::Text(l) => LayerInfo {
                id: l.id,
                name: l.name.clone(),
                layer_type: "text".to_string(),
                position: l.position,
                scale: l.scale,
                opacity: l.opacity,
                frame_range: l.frame_range,
                visible: l.visible,
                text: Some(l.text.clone()),
                font_family: Some(l.font_family.clone()),
                font_size: Some(l.font_size),
                color: Some(l.color),
                stroke: l.stroke.clone(),
                source_width: None,
                source_height: None,
                source_path: None,
            },
        }
    }
}

/// Partial update payload received from the frontend.  Every field is
/// optional; only `Some` fields are applied.
#[derive(serde::Deserialize)]
pub struct LayerUpdate {
    pub name: Option<String>,
    pub position: Option<(f64, f64)>,
    pub scale: Option<f64>,
    pub opacity: Option<f64>,
    pub frame_range: Option<(usize, usize)>,
    pub visible: Option<bool>,
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<[u8; 4]>,
    pub stroke: Option<Stroke>,
}

// ---------------------------------------------------------------------------
// Project
// ---------------------------------------------------------------------------

pub struct Project {
    pub gif: GifData,
    pub layers: Vec<Layer>,
    pub temp_dir: tempfile::TempDir,
}

/// Global app state: at most one project open at a time.
pub type ProjectState = Mutex<Option<Project>>;

impl Project {
    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Open a GIF at `path`, decode its metadata, and set up a temp directory
    /// for frame PNGs.  Returns both the `Project` and the metadata struct
    /// that should be forwarded to the frontend.
    pub fn open(path: &Path) -> Result<(Self, GifMetadata), AppError> {
        let gif = GifData::open(path)?;
        let (width, height) = gif.dimensions();
        let metadata = GifMetadata {
            frame_count: gif.frame_count(),
            width,
            height,
            delays: gif.delays().to_vec(),
        };
        let temp_dir = tempfile::TempDir::new()?;
        let project = Project {
            gif,
            layers: Vec::new(),
            temp_dir,
        };
        Ok((project, metadata))
    }

    // -----------------------------------------------------------------------
    // Frame access
    // -----------------------------------------------------------------------

    /// Return the filesystem path to a PNG of frame `index`.
    ///
    /// The PNG is created on first access and cached by file existence so
    /// subsequent calls are cheap.
    pub fn get_frame_png_path(&mut self, index: usize) -> Result<String, AppError> {
        let png_path: PathBuf = self.temp_dir.path().join(format!("frame_{index:05}.png"));

        if !png_path.exists() {
            let frame: RgbaImage = self.gif.get_frame(index)?;
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
    pub fn add_image_layer(&mut self, path: &str) -> Result<LayerInfo, AppError> {
        let img = image::open(path)
            .map_err(|e| AppError::ImageLoad(e.to_string()))?
            .to_rgba8();

        let (w, h) = img.dimensions();
        let frame_count = self.gif.frame_count();
        let file_name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());

        let mut layer = ImageLayer::new(file_name, w, h);
        layer.image_data = Some(img);
        layer.source_path = Some(path.to_string());
        layer.frame_range = (0, frame_count.saturating_sub(1));

        let info = LayerInfo::from(&Layer::Image(layer.clone()));
        self.layers.push(Layer::Image(layer));
        Ok(info)
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
        let frame_count = self.gif.frame_count();
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
                if let Some(v) = changes.scale {
                    l.scale = v;
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
                // Text fields are silently ignored for image layers.
            }
            Layer::Text(l) => {
                if let Some(v) = changes.name {
                    l.name = v;
                }
                if let Some(v) = changes.position {
                    l.position = v;
                }
                if let Some(v) = changes.scale {
                    l.scale = v;
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

    /// Composite all layers onto the GIF frame at `frame_index`, save the
    /// result as a PNG in the temp directory, and return its path.
    pub fn render_composite(&mut self, frame_index: usize) -> Result<String, AppError> {
        let base: RgbaImage = self.gif.get_frame(frame_index)?;
        let composited = composite_frame(&base, &self.layers, frame_index);

        let out_path: PathBuf = self
            .temp_dir
            .path()
            .join(format!("composite_{frame_index:05}.png"));
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
