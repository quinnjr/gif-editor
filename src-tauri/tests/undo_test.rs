// Undo/redo tests exercising the production AppState::undo / AppState::redo
// methods — the same code paths the Tauri `undo` / `redo` commands delegate to.

use gif_editor_lib::project::{AppState, LayerUpdate, Project, push_history};
use std::path::Path;

fn open_test_gif_state() -> AppState {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test.gif");
    let (project, _) = Project::open(&path).unwrap();
    let mut state = AppState::new();
    state.project = Some(project);
    state
}

#[test]
fn undo_restores_previous_layer_state() {
    let mut state = open_test_gif_state();
    let project = state.project.as_mut().unwrap();
    let layer = project.add_text_layer("original".to_string(), None, None, None, None);

    push_history(&mut state);

    let project = state.project.as_mut().unwrap();
    project
        .update_layer(
            layer.id,
            LayerUpdate {
                text: Some("changed".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let layers = state.undo().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].text.as_deref().unwrap_or(""), "original");
    assert_eq!(state.redo_stack.len(), 1);
}

#[test]
fn redo_reapplies_undone_state() {
    let mut state = open_test_gif_state();
    push_history(&mut state);
    let project = state.project.as_mut().unwrap();
    project.add_text_layer("layer1".to_string(), None, None, None, None);

    let layers = state.undo().unwrap();
    assert_eq!(layers.len(), 0);
    assert_eq!(state.project.as_ref().unwrap().layers.len(), 0);

    let layers = state.redo().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].text.as_deref().unwrap_or(""), "layer1");
}

#[test]
fn undo_with_empty_history_returns_current_layers_unchanged() {
    let mut state = open_test_gif_state();
    let project = state.project.as_mut().unwrap();
    project.add_text_layer("kept".to_string(), None, None, None, None);

    let layers = state.undo().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].text.as_deref().unwrap_or(""), "kept");
    // State is untouched: no snapshot was popped or pushed anywhere.
    assert_eq!(state.project.as_ref().unwrap().layers.len(), 1);
    assert!(state.history.is_empty());
    assert!(state.redo_stack.is_empty());
}

#[test]
fn redo_with_empty_stack_returns_current_layers_unchanged() {
    let mut state = open_test_gif_state();
    let project = state.project.as_mut().unwrap();
    project.add_text_layer("kept".to_string(), None, None, None, None);

    let layers = state.redo().unwrap();
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].text.as_deref().unwrap_or(""), "kept");
    assert!(state.history.is_empty());
    assert!(state.redo_stack.is_empty());
}

#[test]
fn undo_and_redo_with_no_project_return_none() {
    let mut state = AppState::new();
    assert!(state.undo().is_none());
    assert!(state.redo().is_none());
}

#[test]
fn push_history_clears_redo_stack() {
    let mut state = open_test_gif_state();
    push_history(&mut state);
    let project = state.project.as_mut().unwrap();
    project.add_text_layer("layer1".to_string(), None, None, None, None);

    // An undo populates the redo stack…
    state.undo().unwrap();
    assert_eq!(state.redo_stack.len(), 1);

    // …and the next mutation's history push clears it.
    push_history(&mut state);
    assert!(state.redo_stack.is_empty());
}

/// History snapshots must share layer pixel buffers via Arc instead of
/// deep-cloning them: the live layer plus N snapshots all point at the same
/// allocation.
#[test]
fn push_history_shares_image_pixel_buffers() {
    use gif_editor_lib::layer::{ImageLayer, Layer};
    use std::sync::Arc;

    let mut state = open_test_gif_state();
    let project = state.project.as_mut().unwrap();

    let mut layer = ImageLayer::new("img".into(), 4, 4);
    layer.image_data = Some(Arc::new(image::RgbaImage::new(4, 4)));
    layer.frames = Arc::new(vec![image::RgbaImage::new(4, 4)]);
    project.layers.push(Layer::Image(layer));

    for _ in 0..10 {
        push_history(&mut state);
    }

    let project = state.project.as_ref().unwrap();
    let Layer::Image(l) = &project.layers[0] else {
        panic!("expected image layer");
    };
    // Live layer + 10 history snapshots = 11 owners of each shared buffer.
    assert_eq!(Arc::strong_count(l.image_data.as_ref().unwrap()), 11);
    assert_eq!(Arc::strong_count(&l.frames), 11);

    // Undo/redo still round-trip with shared buffers in the history.
    state.undo().unwrap();
    let layers = state.redo().unwrap();
    assert_eq!(layers.len(), 1);
}

#[test]
fn history_capped_at_50() {
    let mut state = open_test_gif_state();
    for _ in 0..55 {
        push_history(&mut state);
    }
    assert_eq!(state.history.len(), 50);
}

#[test]
fn history_cap_drops_oldest_entry() {
    let mut state = open_test_gif_state();

    // Snapshot 0 has zero layers; each subsequent snapshot has one more.
    for i in 0..55 {
        push_history(&mut state);
        let project = state.project.as_mut().unwrap();
        project.add_text_layer(format!("layer{i}"), None, None, None, None);
    }
    assert_eq!(state.history.len(), 50);

    // Undo everything: the deepest reachable snapshot is the one taken after
    // the first 5 layers were added (snapshots 0-4 were dropped by the cap).
    for _ in 0..50 {
        state.undo().unwrap();
    }
    let layers = state.undo().unwrap();
    assert_eq!(layers.len(), 5);
    assert!(state.history.is_empty());
}
