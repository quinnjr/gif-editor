use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub color: [u8; 4],
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub frame: usize,
    pub position: (f64, f64),
    pub opacity: f64,
}

/// Interpolate keyframes for a given frame index.
/// Returns None if keyframes is empty (caller uses base values).
pub fn interpolate_keyframes(
    keyframes: &[Keyframe],
    frame_index: usize,
) -> Option<((f64, f64), f64)> {
    if keyframes.is_empty() {
        return None;
    }
    if frame_index <= keyframes[0].frame {
        let kf = &keyframes[0];
        return Some((kf.position, kf.opacity));
    }
    let last = &keyframes[keyframes.len() - 1];
    if frame_index >= last.frame {
        return Some((last.position, last.opacity));
    }
    for i in 0..keyframes.len() - 1 {
        let a = &keyframes[i];
        let b = &keyframes[i + 1];
        if frame_index >= a.frame && frame_index <= b.frame {
            let span = (b.frame - a.frame) as f64;
            let t = if span > 0.0 {
                (frame_index - a.frame) as f64 / span
            } else {
                0.0
            };
            let x = a.position.0 + t * (b.position.0 - a.position.0);
            let y = a.position.1 + t * (b.position.1 - a.position.1);
            let opacity = a.opacity + t * (b.opacity - a.opacity);
            return Some(((x, y), opacity));
        }
    }
    Some((last.position, last.opacity))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageLayer {
    pub id: Uuid,
    pub name: String,
    #[serde(skip)]
    pub image_data: Option<image::RgbaImage>,
    /// For animated GIF overlays: all decoded frames.  When non-empty the
    /// compositor picks `frames[(project_frame - range_start) % len]`
    /// instead of `image_data`.
    #[serde(skip)]
    pub frames: Vec<image::RgbaImage>,
    pub position: (f64, f64),
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
    pub rotation: f64,
    pub opacity: f64,
    pub frame_range: (usize, usize),
    pub visible: bool,
    pub source_width: u32,
    pub source_height: u32,
    pub source_path: Option<String>,
    pub keyframes: Vec<Keyframe>,
}

impl ImageLayer {
    pub fn new(name: String, width: u32, height: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            image_data: None,
            frames: Vec::new(),
            position: (0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            rotation: 0.0,
            opacity: 1.0,
            frame_range: (0, 0),
            visible: true,
            source_width: width,
            source_height: height,
            source_path: None,
            keyframes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLayer {
    pub id: Uuid,
    pub name: String,
    pub text: String,
    pub font_family: String,
    pub font_size: f64,
    pub color: [u8; 4],
    pub stroke: Option<Stroke>,
    pub text_align: String,
    pub max_width: Option<f64>,
    pub position: (f64, f64),
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
    pub rotation: f64,
    pub opacity: f64,
    pub frame_range: (usize, usize),
    pub visible: bool,
    pub keyframes: Vec<Keyframe>,
}

impl TextLayer {
    pub fn new(text: String) -> Self {
        let name = format!("Text: {}", &text[..text.len().min(20)]);
        Self {
            id: Uuid::new_v4(),
            name,
            text,
            font_family: "Impact".to_string(),
            font_size: 48.0,
            color: [255, 255, 255, 255],
            stroke: Some(Stroke {
                color: [0, 0, 0, 255],
                width: 2.0,
            }),
            text_align: "center".to_string(),
            max_width: None,
            position: (0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            rotation: 0.0,
            opacity: 1.0,
            frame_range: (0, 0),
            visible: true,
            keyframes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Layer {
    Image(ImageLayer),
    Text(TextLayer),
}

impl Layer {
    pub fn id(&self) -> Uuid {
        match self {
            Layer::Image(l) => l.id,
            Layer::Text(l) => l.id,
        }
    }

    pub fn visible(&self) -> bool {
        match self {
            Layer::Image(l) => l.visible,
            Layer::Text(l) => l.visible,
        }
    }

    pub fn frame_range(&self) -> (usize, usize) {
        match self {
            Layer::Image(l) => l.frame_range,
            Layer::Text(l) => l.frame_range,
        }
    }

    pub fn keyframes(&self) -> &[Keyframe] {
        match self {
            Layer::Image(l) => &l.keyframes,
            Layer::Text(l) => &l.keyframes,
        }
    }

    pub fn scale_x_val(&self) -> f64 {
        match self {
            Layer::Image(l) => l.scale_x,
            Layer::Text(l) => l.scale_x,
        }
    }

    pub fn scale_y_val(&self) -> f64 {
        match self {
            Layer::Image(l) => l.scale_y,
            Layer::Text(l) => l.scale_y,
        }
    }
}
