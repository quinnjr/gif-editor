// Tauri IPC command handlers.
//
// Every public function here is decorated with #[tauri::command] and
// registered in lib.rs via generate_handler![].  Each handler extracts the
// ProjectState mutex, delegates to Project methods, and converts errors into
// the serialisable AppError type automatically.

use std::path::Path;

use tauri::{Emitter, State};
use uuid::Uuid;

use crate::error::AppError;
use crate::export::{self, ExportSettings};
use crate::layer::Stroke;
use crate::project::{GifMetadata, LayerInfo, LayerUpdate, Project, ProjectState};

/// Open a media file (GIF, MP4, or WebM) and initialise project state.
/// Returns metadata so the frontend can set up its frame timeline immediately.
#[tauri::command]
pub async fn open_file(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let (project, metadata) = Project::open(Path::new(&path))?;
    *state.lock().unwrap() = Some(project);
    Ok(metadata)
}

/// Backwards-compatible alias for `open_file`.
#[tauri::command]
pub async fn open_gif(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    open_file(path, state).await
}

/// Return the filesystem path to a decoded PNG for `frame_index`.
///
/// The PNG is created lazily and cached; subsequent calls for the same index
/// are cheap.
#[tauri::command]
pub async fn get_frame(
    frame_index: usize,
    state: State<'_, ProjectState>,
) -> Result<String, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.get_frame_png_path(frame_index)
}

/// Load an image from `path` and add it as a new layer on top of the stack.
#[tauri::command]
pub async fn add_image_layer(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.add_image_layer(&path)
}

/// Create a new text layer with optional style overrides.
#[tauri::command]
pub async fn add_text_layer(
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
pub async fn update_layer(
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
pub async fn remove_layer(
    id: Uuid,
    state: State<'_, ProjectState>,
) -> Result<(), AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.remove_layer(id)
}

/// Reorder the layer stack to match the supplied list of layer ids.
#[tauri::command]
pub async fn reorder_layers(
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
pub async fn render_composite(
    frame_index: usize,
    state: State<'_, ProjectState>,
) -> Result<String, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.render_composite(frame_index)
}

/// Return the current layer stack in order.
#[tauri::command]
pub async fn get_layers(state: State<'_, ProjectState>) -> Result<Vec<LayerInfo>, AppError> {
    let guard = state.lock().unwrap();
    let project = guard.as_ref().ok_or(AppError::NoProject)?;
    Ok(project.get_layers())
}

/// Return the list of font families available to the text renderer.
#[tauri::command]
pub fn get_system_fonts() -> Vec<String> {
    crate::fonts::list_available_fonts()
}

/// Export the current project to a file.
///
/// The export format is taken from `settings.format`.  Progress events are
/// emitted on the "export-progress" channel as a plain frame count so the
/// frontend can drive a progress bar without polling.
#[tauri::command]
pub async fn export_project(
    state: State<'_, ProjectState>,
    app: tauri::AppHandle,
    settings: ExportSettings,
    output_path: String,
) -> Result<(), AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;

    let out = std::path::Path::new(&output_path);
    let layers = project.layers.clone();

    let on_progress = |frames_done: usize| {
        let _ = app.emit("export-progress", frames_done);
    };

    match settings.format {
        export::ExportFormat::Gif => {
            export::export_gif(project.source.as_mut(), &layers, &settings, out, on_progress)
        }
        export::ExportFormat::Mp4 | export::ExportFormat::WebM => {
            export::export_video(project.source.as_mut(), &layers, &settings, out, on_progress)
        }
    }
}

/// Return `true` if ffmpeg is available on PATH.
///
/// The frontend uses this to decide whether to offer video export options.
#[tauri::command]
pub fn check_ffmpeg() -> bool {
    export::ffmpeg_available()
}
