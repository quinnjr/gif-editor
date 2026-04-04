use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub color: [u8; 4],
    pub width: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageLayer {
    pub id: Uuid,
    pub name: String,
    #[serde(skip)]
    pub image_data: Option<image::RgbaImage>,
    pub position: (f64, f64),
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
    pub opacity: f64,
    pub frame_range: (usize, usize),
    pub visible: bool,
    pub source_width: u32,
    pub source_height: u32,
    pub source_path: Option<String>,
}

impl ImageLayer {
    pub fn new(name: String, width: u32, height: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            image_data: None,
            position: (0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            opacity: 1.0,
            frame_range: (0, 0),
            visible: true,
            source_width: width,
            source_height: height,
            source_path: None,
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
    pub position: (f64, f64),
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
    pub opacity: f64,
    pub frame_range: (usize, usize),
    pub visible: bool,
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
            position: (0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            opacity: 1.0,
            frame_range: (0, 0),
            visible: true,
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
}
