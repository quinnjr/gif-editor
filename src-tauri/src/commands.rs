// Tauri IPC command handlers.
//
// Every public function here is decorated with #[tauri::command] and
// registered in lib.rs via generate_handler![].  Each handler extracts the
// ProjectState mutex, delegates to Project methods, and converts errors into
// the serialisable AppError type automatically.

use std::path::Path;

use tauri::State;
use uuid::Uuid;

use crate::error::AppError;
use crate::layer::Stroke;
use crate::project::{GifMetadata, LayerInfo, LayerUpdate, Project, ProjectState};

/// Open a GIF file and initialise project state.  Returns metadata about the
/// GIF so the frontend can set up its frame timeline immediately.
#[tauri::command]
pub fn open_gif(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let (project, metadata) = Project::open(Path::new(&path))?;
    *state.lock().unwrap() = Some(project);
    Ok(metadata)
}

/// Return the filesystem path to a decoded PNG for `frame_index`.
///
/// The PNG is created lazily and cached; subsequent calls for the same index
/// are cheap.
#[tauri::command]
pub fn get_frame(
    frame_index: usize,
    state: State<'_, ProjectState>,
) -> Result<String, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.get_frame_png_path(frame_index)
}

/// Load an image from `path` and add it as a new layer on top of the stack.
#[tauri::command]
pub fn add_image_layer(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.add_image_layer(&path)
}

/// Create a new text layer with optional style overrides.
#[tauri::command]
pub fn add_text_layer(
    text: String,
    font_family: Option<String>,
    font_size: Option<f64>,
    color: Option<[u8; 4]>,
    stroke: Option<Stroke>,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    Ok(project.add_text_layer(text, font_family, font_size, color, stroke))
}

/// Apply a partial update to the layer identified by `id`.
#[tauri::command]
pub fn update_layer(
    id: Uuid,
    changes: LayerUpdate,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.update_layer(id, changes)
}

/// Remove the layer with the given `id` from the stack.
#[tauri::command]
pub fn remove_layer(
    id: Uuid,
    state: State<'_, ProjectState>,
) -> Result<(), AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.remove_layer(id)
}

/// Reorder the layer stack to match the supplied list of layer ids.
#[tauri::command]
pub fn reorder_layers(
    ids: Vec<Uuid>,
    state: State<'_, ProjectState>,
) -> Result<(), AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.reorder_layers(ids)
}

/// Composite all layers onto frame `frame_index`, write the result as a PNG,
/// and return the path so the frontend can display it.
#[tauri::command]
pub fn render_composite(
    frame_index: usize,
    state: State<'_, ProjectState>,
) -> Result<String, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.render_composite(frame_index)
}

/// Return the current layer stack in order.
#[tauri::command]
pub fn get_layers(state: State<'_, ProjectState>) -> Result<Vec<LayerInfo>, AppError> {
    let guard = state.lock().unwrap();
    let project = guard.as_ref().ok_or(AppError::NoProject)?;
    Ok(project.get_layers())
}

/// Return the list of font families available to the text renderer.
#[tauri::command]
pub fn get_system_fonts() -> Vec<String> {
    crate::fonts::list_available_fonts()
}
