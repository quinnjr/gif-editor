use gif_editor_lib::project::{push_history, AppState, LayerUpdate, Project};
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
    project.update_layer(layer.id, LayerUpdate {
        text: Some("changed".to_string()),
        ..Default::default()
    }).unwrap();
    assert_eq!(state.project.as_ref().unwrap().layers.len(), 1);

    // Undo
    let entry = state.history.pop().unwrap();
    let redo_entry = gif_editor_lib::project::HistoryEntry {
        layers: state.project.as_ref().unwrap().layers.clone(),
        excluded_frames: state.project.as_ref().unwrap().excluded_frames.clone(),
    };
    state.redo_stack.push(redo_entry);
    let project = state.project.as_mut().unwrap();
    project.layers = entry.layers;
    project.excluded_frames = entry.excluded_frames;

    let layers = state.project.as_ref().unwrap().get_layers();
    assert_eq!(
        layers[0].text.as_deref().unwrap_or(""),
        "original"
    );
}

#[test]
fn redo_reapplies_undone_state() {
    let mut state = open_test_gif_state();
    push_history(&mut state);
    let project = state.project.as_mut().unwrap();
    project.add_text_layer("layer1".to_string(), None, None, None, None);

    // Undo
    let entry = state.history.pop().unwrap();
    let redo_entry = gif_editor_lib::project::HistoryEntry {
        layers: state.project.as_ref().unwrap().layers.clone(),
        excluded_frames: state.project.as_ref().unwrap().excluded_frames.clone(),
    };
    state.redo_stack.push(redo_entry);
    let project = state.project.as_mut().unwrap();
    project.layers = entry.layers;
    project.excluded_frames = entry.excluded_frames;
    assert_eq!(state.project.as_ref().unwrap().layers.len(), 0);

    // Redo
    let redo = state.redo_stack.pop().unwrap();
    state.history.push(gif_editor_lib::project::HistoryEntry {
        layers: state.project.as_ref().unwrap().layers.clone(),
        excluded_frames: state.project.as_ref().unwrap().excluded_frames.clone(),
    });
    let project = state.project.as_mut().unwrap();
    project.layers = redo.layers;
    project.excluded_frames = redo.excluded_frames;
    assert_eq!(state.project.as_ref().unwrap().layers.len(), 1);
}

#[test]
fn push_history_clears_redo_stack() {
    let mut state = open_test_gif_state();
    push_history(&mut state);
    state.redo_stack.push(gif_editor_lib::project::HistoryEntry {
        layers: vec![],
        excluded_frames: Default::default(),
    });
    push_history(&mut state);
    assert!(state.redo_stack.is_empty());
}

#[test]
fn history_capped_at_50() {
    let mut state = open_test_gif_state();
    for _ in 0..55 {
        push_history(&mut state);
    }
    assert_eq!(state.history.len(), 50);
}
