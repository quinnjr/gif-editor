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
use crate::project::{GifMetadata, LayerInfo, LayerUpdate, Project, ProjectState, push_history};

/// Open a media file (GIF, MP4, or WebM) and initialise project state.
/// Returns metadata so the frontend can set up its frame timeline immediately.
#[tauri::command]
pub async fn open_file(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let (project, metadata) = Project::open(Path::new(&path))?;
    let mut guard = state.lock().unwrap();
    guard.project = Some(project);
    guard.history.clear();
    guard.redo_stack.clear();
    Ok(metadata)
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
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.get_frame_png_path(frame_index)
}

/// Load an image or animated GIF as a new layer.
///
/// When an animated GIF is added on top of a static image source, the
/// project timeline expands to fit the GIF frames.  The returned tuple
/// contains the new layer info and optionally refreshed metadata (if the
/// timeline changed).
#[tauri::command]
pub async fn add_image_layer(
    path: String,
    state: State<'_, ProjectState>,
) -> Result<(LayerInfo, Option<GifMetadata>), AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
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
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    Ok(project.add_text_layer(text, font_family, font_size, color, stroke))
}

/// Create a new solar flare layer at `position` (defaults to canvas centre).
#[tauri::command]
pub async fn add_flare_layer(
    position: Option<(f64, f64)>,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    Ok(project.add_flare_layer(position))
}

/// Apply a partial update to the layer identified by `id`.
#[tauri::command]
pub async fn update_layer(
    id: Uuid,
    changes: LayerUpdate,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.update_layer(id, changes)
}

/// Remove the layer with the given `id` from the stack.
#[tauri::command]
pub async fn remove_layer(id: Uuid, state: State<'_, ProjectState>) -> Result<(), AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.remove_layer(id)
}

/// Reorder the layer stack to match the supplied list of layer ids.
#[tauri::command]
pub async fn reorder_layers(
    ids: Vec<Uuid>,
    state: State<'_, ProjectState>,
) -> Result<(), AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
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
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.render_composite(frame_index)
}

/// Return the current layer stack in order.
#[tauri::command]
pub async fn get_layers(state: State<'_, ProjectState>) -> Result<Vec<LayerInfo>, AppError> {
    let guard = state.lock().unwrap();
    let project = guard.project.as_ref().ok_or(AppError::NoProject)?;
    Ok(project.get_layers())
}

/// Soft-delete frames by logical index.  Returns updated metadata reflecting
/// the new visible frame count and delay list.
#[tauri::command]
pub async fn delete_frames(
    indices: Vec<usize>,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.delete_frames(&indices)
}

/// Restore previously excluded frames by source index.  Returns updated
/// metadata reflecting the new visible frame count and delay list.
#[tauri::command]
pub async fn restore_frames(
    source_indices: Vec<usize>,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.restore_frames(&source_indices)
}

/// Return the set of source frame indices currently excluded from the
/// visible timeline.
#[tauri::command]
pub async fn get_excluded_frames(state: State<'_, ProjectState>) -> Result<Vec<usize>, AppError> {
    let guard = state.lock().unwrap();
    let project = guard.project.as_ref().ok_or(AppError::NoProject)?;
    Ok(project.get_excluded_frames())
}

/// Return the bundled font families available to the text renderer.
///
/// No system font enumeration is performed; the list is the fixed set of
/// fonts compiled into the binary.
#[tauri::command]
pub fn get_available_fonts() -> Vec<String> {
    crate::fonts::list_available_fonts()
}

/// Export the current project to a file.
///
/// The export format is taken from `settings.format`.  Progress events are
/// emitted on the "export-progress" channel as a plain frame count so the
/// frontend can drive a progress bar without polling.
///
/// The state mutex is NOT held for the duration of the export:
/// `Project::take_source_for_export` snapshots everything under the lock and
/// moves the frame source out of the project (replaced with an
/// `ExportingPlaceholder`), so other commands — layer edits, undo/redo,
/// cached-frame scrubbing — keep working while frames are encoded.  Fetching
/// an uncached frame during the export errors with "export in progress"
/// (surfaced by the frontend as a toast).  The real source is restored
/// afterwards even if the export panics, so a crash mid-export cannot strand
/// the placeholder.
#[tauri::command]
pub async fn export_project(
    state: State<'_, ProjectState>,
    app: tauri::AppHandle,
    settings: ExportSettings,
    output_path: String,
) -> Result<(), AppError> {
    let out = std::path::Path::new(&output_path);

    // Phase 1 (locked): snapshot the export inputs and swap the frame source
    // for a placeholder, then release the lock.
    let mut snapshot = {
        let mut guard = state.lock().unwrap();
        let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
        project.take_source_for_export(&settings)?
    };

    let on_progress = |frames_done: usize| {
        let _ = app.emit("export-progress", frames_done);
    };

    // Phase 2 (unlocked): run the export against the owned source.  Panics
    // are caught (and re-raised below) so the restore runs on the unwind
    // path too — otherwise the placeholder would be stranded and every later
    // frame access would fail with "export in progress".
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        export::run_export(&mut snapshot, &settings, out, on_progress)
    }));

    // Phase 3 (locked): restore the real source while our placeholder is
    // still installed (a project opened mid-export is not ours to replace).
    {
        let mut guard = state.lock().unwrap();
        if let Some(project) = guard.project.as_mut() {
            project.restore_source_if_placeholder(snapshot.source);
        }
    }

    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// Return `true` if ffmpeg is available on PATH.
///
/// The frontend uses this to decide whether to offer video export options.
#[tauri::command]
pub fn check_ffmpeg() -> bool {
    export::ffmpeg_available()
}

/// Undo the last mutating action. Returns the updated layer list.
///
/// Never errors on missing history: with a project open and an empty history
/// stack it returns the current layer list unchanged, and with no project
/// open it returns an empty Vec.
#[tauri::command]
pub async fn undo(state: State<'_, ProjectState>) -> Result<Vec<LayerInfo>, AppError> {
    let mut guard = state.lock().unwrap();
    Ok(guard.undo().unwrap_or_default())
}

/// Redo the last undone action. Returns the updated layer list.
///
/// Never errors on missing history: with a project open and an empty redo
/// stack it returns the current layer list unchanged, and with no project
/// open it returns an empty Vec.
#[tauri::command]
pub async fn redo(state: State<'_, ProjectState>) -> Result<Vec<LayerInfo>, AppError> {
    let mut guard = state.lock().unwrap();
    Ok(guard.redo().unwrap_or_default())
}

/// Flip a layer along `axis` ("horizontal" or "vertical") by negating scale.
#[tauri::command]
pub async fn flip_layer(
    id: Uuid,
    axis: String,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.flip_layer(id, &axis)
}

/// Clone the layer identified by `id` and insert the copy above it in the stack.
#[tauri::command]
pub async fn duplicate_layer(
    id: Uuid,
    state: State<'_, ProjectState>,
) -> Result<LayerInfo, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.duplicate_layer(id)
}

/// Scale all layers by the given factors.
#[tauri::command]
pub async fn scale_all_layers(
    scale_x: f64,
    scale_y: f64,
    state: State<'_, ProjectState>,
) -> Result<Vec<LayerInfo>, AppError> {
    let mut guard = state.lock().unwrap();
    push_history(&mut guard);
    let project = guard.project.as_mut().ok_or(AppError::NoProject)?;
    project.scale_all_layers(scale_x, scale_y)
}
