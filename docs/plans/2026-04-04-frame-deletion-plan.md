# Frame Deletion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add soft-delete frame elimination with multi-select, range selection, restore, and auto-adjusting layer ranges.

**Architecture:** Add `excluded_frames: BTreeSet<usize>` to Project with logical↔source index mapping. All frame access goes through the mapping. Three new Tauri commands (delete_frames, restore_frames, get_excluded_frames). Timeline gets multi-select UI with Delete/Keep/Restore actions.

**Tech Stack:** Rust BTreeSet for ordered exclusion set, Svelte 5 reactive state for selection, existing Tauri IPC pattern.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/project.rs` | Modify | Add `excluded_frames`, mapping helpers, update metadata/frame access |
| `src-tauri/src/commands.rs` | Modify | Add `delete_frames`, `restore_frames`, `get_excluded_frames` commands |
| `src-tauri/src/lib.rs` | Modify | Register 3 new commands |
| `src-tauri/src/export.rs` | Modify | Change export loops to use visible frame indices |
| `src-tauri/src/error.rs` | Modify | Add `FrameDeletion` error variant |
| `src-tauri/tests/project_test.rs` | Create | Tests for frame mapping, deletion, restoration, layer adjustment |
| `src/lib/commands.ts` | Modify | Add 3 new command wrappers |
| `src/lib/stores/project.svelte.ts` | Modify | Add deleteFrames, restoreFrames, excluded state |
| `src/lib/components/Timeline.svelte` | Modify | Multi-select, Delete/Keep/Restore UI |

---

### Task 1: Add Exclusion Set and Mapping Helpers to Project

**Files:**
- Modify: `src-tauri/src/project.rs`
- Modify: `src-tauri/src/error.rs`
- Create: `src-tauri/tests/project_test.rs`

- [ ] **Step 1: Add error variant**

In `src-tauri/src/error.rs`, add after the `LayerNotFound` variant:

```rust
    #[error("Frame deletion error: {0}")]
    FrameDeletion(String),
```

- [ ] **Step 2: Add `excluded_frames` field and imports to Project**

In `src-tauri/src/project.rs`, add to the imports at the top:

```rust
use std::collections::BTreeSet;
```

In the `Project` struct, add a new field after `temp_dir`:

```rust
pub struct Project {
    pub source: Box<dyn FrameSource>,
    pub layers: Vec<Layer>,
    pub temp_dir: tempfile::TempDir,
    pub excluded_frames: BTreeSet<usize>,
}
```

In `Project::open`, update the project construction:

```rust
        let project = Project {
            source,
            layers: Vec::new(),
            temp_dir,
            excluded_frames: BTreeSet::new(),
        };
```

- [ ] **Step 3: Add mapping helpers and visible metadata methods**

Add these methods to `impl Project` after the `open` method (before the "Frame access" section):

```rust
    // -----------------------------------------------------------------------
    // Frame index mapping
    // -----------------------------------------------------------------------

    /// Number of frames visible to the user (source count minus excluded).
    pub fn visible_frame_count(&self) -> usize {
        self.source.frame_count() - self.excluded_frames.len()
    }

    /// Delays for visible frames only, in logical order.
    pub fn visible_delays(&self) -> Vec<u16> {
        let all_delays = self.source.delays();
        (0..self.source.frame_count())
            .filter(|i| !self.excluded_frames.contains(i))
            .map(|i| all_delays[i])
            .collect()
    }

    /// Build a GifMetadata reflecting only visible frames.
    pub fn visible_metadata(&self) -> GifMetadata {
        let (width, height) = self.source.dimensions();
        GifMetadata {
            frame_count: self.visible_frame_count(),
            width,
            height,
            delays: self.visible_delays(),
        }
    }

    /// Map a logical (displayed) frame index to its source index.
    /// Returns None if the logical index is out of bounds.
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

    /// Map a source frame index to its logical (displayed) index.
    /// Returns None if the source index is excluded.
    pub fn source_to_logical(&self, source: usize) -> Option<usize> {
        if self.excluded_frames.contains(&source) {
            return None;
        }
        let logical = (0..source)
            .filter(|i| !self.excluded_frames.contains(i))
            .count();
        Some(logical)
    }
```

- [ ] **Step 4: Update `get_frame_png_path` and `render_composite` to use mapping**

Replace `get_frame_png_path`:

```rust
    pub fn get_frame_png_path(&mut self, logical_index: usize) -> Result<String, AppError> {
        let src_index = self.logical_to_source(logical_index).ok_or_else(|| {
            AppError::FrameDeletion(format!(
                "logical frame {logical_index} out of bounds (visible={})",
                self.visible_frame_count()
            ))
        })?;
        let png_path: PathBuf = self.temp_dir.path().join(format!("frame_{src_index:05}.png"));

        if !png_path.exists() {
            let frame: RgbaImage = self.source.get_frame(src_index)?;
            frame
                .save(&png_path)
                .map_err(|e| AppError::Export(e.to_string()))?;
        }

        Ok(png_path.to_string_lossy().into_owned())
    }
```

Replace `render_composite`:

```rust
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
```

- [ ] **Step 5: Update `open` to return visible metadata**

In `Project::open`, replace the metadata construction:

```rust
        let (width, height) = source.dimensions();
        let metadata = GifMetadata {
            frame_count: source.frame_count(),
            width,
            height,
            delays: source.delays().to_vec(),
        };
```

with:

```rust
        let (width, height) = source.dimensions();
```

And change the return to construct metadata from the project:

```rust
        let project = Project {
            source,
            layers: Vec::new(),
            temp_dir,
            excluded_frames: BTreeSet::new(),
        };
        let metadata = project.visible_metadata();
        Ok((project, metadata))
```

- [ ] **Step 6: Update `add_image_layer` and `add_text_layer` to use visible count**

In `add_image_layer`, replace:
```rust
        let frame_count = self.source.frame_count();
```
with:
```rust
        let frame_count = self.visible_frame_count();
```

In `add_text_layer`, make the same replacement.

- [ ] **Step 7: Write tests**

Create `src-tauri/tests/project_test.rs`:

```rust
use gif_editor_lib::project::Project;
use std::path::Path;

fn open_test_gif() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/test.gif");
    let (project, _meta) = Project::open(&path).unwrap();
    project
}

#[test]
fn logical_to_source_identity_when_no_exclusions() {
    let project = open_test_gif();
    let count = project.visible_frame_count();
    assert!(count > 0);
    for i in 0..count {
        assert_eq!(project.logical_to_source(i), Some(i));
    }
    assert_eq!(project.logical_to_source(count), None);
}

#[test]
fn source_to_logical_identity_when_no_exclusions() {
    let project = open_test_gif();
    let count = project.source.frame_count();
    for i in 0..count {
        assert_eq!(project.source_to_logical(i), Some(i));
    }
}

#[test]
fn logical_to_source_skips_excluded() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3, "test GIF needs at least 3 frames");

    project.excluded_frames.insert(1);
    // logical 0 -> source 0
    assert_eq!(project.logical_to_source(0), Some(0));
    // logical 1 -> source 2 (skipped 1)
    assert_eq!(project.logical_to_source(1), Some(2));
    assert_eq!(project.visible_frame_count(), total - 1);
}

#[test]
fn source_to_logical_returns_none_for_excluded() {
    let mut project = open_test_gif();
    project.excluded_frames.insert(1);
    assert_eq!(project.source_to_logical(0), Some(0));
    assert_eq!(project.source_to_logical(1), None);
    assert_eq!(project.source_to_logical(2), Some(1));
}

#[test]
fn visible_delays_excludes_hidden_frames() {
    let mut project = open_test_gif();
    let all_delays = project.source.delays().to_vec();
    let total = all_delays.len();
    assert!(total >= 3);

    project.excluded_frames.insert(1);
    let visible = project.visible_delays();
    assert_eq!(visible.len(), total - 1);
    assert_eq!(visible[0], all_delays[0]);
    assert_eq!(visible[1], all_delays[2]); // skipped index 1
}

#[test]
fn visible_metadata_reflects_exclusions() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    project.excluded_frames.insert(0);
    let meta = project.visible_metadata();
    assert_eq!(meta.frame_count, total - 1);
    assert_eq!(meta.delays.len(), total - 1);
}
```

- [ ] **Step 8: Check for a test GIF fixture**

Run: `ls src-tauri/tests/fixtures/`

If `test.gif` does not exist, create a small test GIF:

Run: `ffmpeg -f lavfi -i "color=c=red:s=4x4:d=0.3,fps=10" -loop 0 src-tauri/tests/fixtures/test.gif 2>/dev/null`

This creates a 3-frame 4×4 red GIF.

- [ ] **Step 9: Run tests**

Run: `cargo test --test project_test -v`

Expected: all 6 tests pass.

- [ ] **Step 10: Commit**

```
feat(project): add frame exclusion set with logical/source index mapping

Project now tracks excluded_frames as a BTreeSet of source indices.
Mapping helpers translate between logical (displayed) and source
indices. get_frame_png_path and render_composite go through the
mapping so excluded frames are invisible to the frontend.

Metadata, delays, and frame counts reflect only visible frames.
Layer creation uses visible_frame_count for default frame ranges.
```

---

### Task 2: Delete and Restore Commands

**Files:**
- Modify: `src-tauri/src/project.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Add to: `src-tauri/tests/project_test.rs`

- [ ] **Step 1: Add delete_frames and restore_frames methods to Project**

Add to `impl Project`, after the mapping helpers:

```rust
    // -----------------------------------------------------------------------
    // Frame deletion / restoration
    // -----------------------------------------------------------------------

    /// Soft-delete frames at the given logical indices.
    ///
    /// Returns an error if deleting would remove all frames.
    /// Auto-adjusts layer frame_range values to preserve alignment.
    pub fn delete_frames(&mut self, logical_indices: &[usize]) -> Result<GifMetadata, AppError> {
        // Convert logical to source before modifying exclusions.
        let mut source_indices: Vec<usize> = Vec::new();
        for &li in logical_indices {
            if let Some(si) = self.logical_to_source(li) {
                source_indices.push(si);
            }
        }

        // Guard: must keep at least one frame.
        let new_excluded_count = self.excluded_frames.len() + source_indices.len();
        if new_excluded_count >= self.source.frame_count() {
            return Err(AppError::FrameDeletion(
                "cannot delete all frames; at least one must remain".to_string(),
            ));
        }

        // Snapshot layer ranges as source indices before modification.
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

        // Apply exclusions.
        for si in &source_indices {
            self.excluded_frames.insert(*si);
        }

        // Re-map layer ranges.
        self.remap_layer_ranges(&layer_source_ranges);

        Ok(self.visible_metadata())
    }

    /// Restore previously-excluded frames by their source indices.
    ///
    /// Auto-adjusts layer frame_range values.
    pub fn restore_frames(&mut self, source_indices: &[usize]) -> Result<GifMetadata, AppError> {
        // Snapshot layer ranges as source indices before modification.
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

        for si in source_indices {
            self.excluded_frames.remove(si);
        }

        self.remap_layer_ranges(&layer_source_ranges);

        Ok(self.visible_metadata())
    }

    /// Return the set of excluded source indices.
    pub fn get_excluded_frames(&self) -> Vec<usize> {
        self.excluded_frames.iter().copied().collect()
    }

    /// Re-map layer frame ranges from source indices to new logical indices
    /// after the exclusion set has changed.
    fn remap_layer_ranges(&mut self, source_ranges: &[(usize, usize)]) {
        let visible_count = self.visible_frame_count();
        for (i, layer) in self.layers.iter_mut().enumerate() {
            let (src_start, src_end) = source_ranges[i];

            // Find nearest non-excluded source index for each endpoint.
            let new_start = self
                .source_to_logical(src_start)
                .or_else(|| self.find_nearest_logical(src_start, true));
            let new_end = self
                .source_to_logical(src_end)
                .or_else(|| self.find_nearest_logical(src_end, false));

            let (ns, ne) = match (new_start, new_end) {
                (Some(s), Some(e)) if s <= e => (s, e),
                (Some(s), Some(_)) => (s, s), // collapsed range
                _ => (0, visible_count.saturating_sub(1)),
            };

            match layer {
                Layer::Image(l) => l.frame_range = (ns, ne),
                Layer::Text(l) => l.frame_range = (ns, ne),
            }
        }
    }

    /// Find the logical index of the nearest non-excluded source frame.
    /// If `search_forward` is true, search forward from `source`; otherwise backward.
    fn find_nearest_logical(&self, source: usize, search_forward: bool) -> Option<usize> {
        let total = self.source.frame_count();
        if search_forward {
            for s in source..total {
                if let Some(l) = self.source_to_logical(s) {
                    return Some(l);
                }
            }
            // Fallback: search backward
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
            // Fallback: search forward
            for s in source..total {
                if let Some(l) = self.source_to_logical(s) {
                    return Some(l);
                }
            }
        }
        None
    }
```

- [ ] **Step 2: Add Tauri commands**

In `src-tauri/src/commands.rs`, add after `get_layers`:

```rust
/// Soft-delete frames at the given logical indices.
/// Returns refreshed metadata reflecting the remaining visible frames.
#[tauri::command]
pub async fn delete_frames(
    indices: Vec<usize>,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.delete_frames(&indices)
}

/// Restore previously-excluded frames by their source indices.
/// Returns refreshed metadata.
#[tauri::command]
pub async fn restore_frames(
    source_indices: Vec<usize>,
    state: State<'_, ProjectState>,
) -> Result<GifMetadata, AppError> {
    let mut guard = state.lock().unwrap();
    let project = guard.as_mut().ok_or(AppError::NoProject)?;
    project.restore_frames(&source_indices)
}

/// Return the list of excluded source frame indices.
#[tauri::command]
pub async fn get_excluded_frames(
    state: State<'_, ProjectState>,
) -> Result<Vec<usize>, AppError> {
    let guard = state.lock().unwrap();
    let project = guard.as_ref().ok_or(AppError::NoProject)?;
    Ok(project.get_excluded_frames())
}
```

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the `generate_handler!` list:

```rust
            commands::delete_frames,
            commands::restore_frames,
            commands::get_excluded_frames,
```

- [ ] **Step 4: Add tests for delete and restore**

Append to `src-tauri/tests/project_test.rs`:

```rust
use gif_editor_lib::layer::{Layer, TextLayer};

#[test]
fn delete_frames_excludes_and_adjusts_metadata() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);

    let meta = project.delete_frames(&[1]).unwrap();
    assert_eq!(meta.frame_count, total - 1);
    assert_eq!(project.excluded_frames.len(), 1);
    assert!(project.excluded_frames.contains(&1));
}

#[test]
fn delete_all_frames_rejected() {
    let mut project = open_test_gif();
    let total = project.visible_frame_count();
    let all: Vec<usize> = (0..total).collect();
    let result = project.delete_frames(&all);
    assert!(result.is_err());
}

#[test]
fn restore_frames_brings_back_excluded() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    project.delete_frames(&[0, 1]).unwrap();
    assert_eq!(project.visible_frame_count(), total - 2);

    let meta = project.restore_frames(&[0]).unwrap();
    assert_eq!(meta.frame_count, total - 1);
    assert!(!project.excluded_frames.contains(&0));
    assert!(project.excluded_frames.contains(&1));
}

#[test]
fn delete_adjusts_layer_frame_ranges() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);

    // Add a text layer covering all frames
    let mut layer = TextLayer::new("test".to_string());
    layer.frame_range = (0, total - 1);
    project.layers.push(Layer::Text(layer));

    // Delete frame 0 — layer should shift down
    project.delete_frames(&[0]).unwrap();
    let range = project.layers[0].frame_range();
    // After deleting source frame 0, source frame 1 is now logical 0,
    // and the last frame shifts down by 1.
    assert_eq!(range, (0, total - 2));
}

#[test]
fn restore_adjusts_layer_frame_ranges() {
    let mut project = open_test_gif();
    let total = project.source.frame_count();
    assert!(total >= 3);

    let mut layer = TextLayer::new("test".to_string());
    layer.frame_range = (0, total - 1);
    project.layers.push(Layer::Text(layer));

    project.delete_frames(&[0]).unwrap();
    // Now restore it
    project.restore_frames(&[0]).unwrap();
    let range = project.layers[0].frame_range();
    assert_eq!(range, (0, total - 1));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test project_test -v`

Expected: all 11 tests pass (6 from Task 1 + 5 new).

- [ ] **Step 6: Commit**

```
feat(project): add delete_frames and restore_frames with layer auto-adjustment

Frames can be soft-deleted by logical index and restored by source
index. Layer frame_range values are automatically remapped when the
exclusion set changes to preserve alignment with the same source
content. At least one frame must remain visible.

Three new Tauri commands: delete_frames, restore_frames,
get_excluded_frames.
```

---

### Task 3: Update Export Pipeline

**Files:**
- Modify: `src-tauri/src/export.rs`
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Change export functions to accept a frame index list**

The export functions currently iterate `0..frame_count` on the raw FrameSource. They need to accept a list of source indices to export and the matching delays.

In `src-tauri/src/export.rs`, change `export_gif` signature:

```rust
pub fn export_gif(
    source: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    frame_indices: &[usize],
    delays: &[u16],
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
```

Replace the frame count and loop:

```rust
    let frame_count = frame_indices.len();
    let (src_w, src_h) = source.dimensions();
    let (out_w, out_h) = settings.resize.unwrap_or((src_w, src_h));
```

Replace the `for i in 0..frame_count` loop body:

```rust
    for (logical, &src_i) in frame_indices.iter().enumerate() {
        let base = source.get_frame(src_i)?;
        let composited = compositor::composite_frame(&base, layers, logical);

        let final_img = if (out_w, out_h) != (src_w, src_h) {
            imageops::resize(&composited, out_w, out_h, imageops::FilterType::Lanczos3)
        } else {
            composited
        };

        // Quantise RGBA to a 256-colour palette with imagequant.
        let mut iq = imagequant::new();
        iq.set_quality(0, iq_quality)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let pixels: Vec<imagequant::RGBA> = final_img
            .pixels()
            .map(|p| imagequant::RGBA {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            })
            .collect();

        let mut iq_image = iq
            .new_image(
                pixels.as_slice(),
                out_w as usize,
                out_h as usize,
                0.0,
            )
            .map_err(|e| AppError::Export(e.to_string()))?;

        let mut res = iq
            .quantize(&mut iq_image)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let (palette, indices) = res
            .remapped(&mut iq_image)
            .map_err(|e| AppError::Export(e.to_string()))?;

        let palette_bytes: Vec<u8> = palette
            .iter()
            .flat_map(|c| [c.r, c.g, c.b])
            .collect();

        let delay = delays[logical];
        let mut frame = gif::Frame::from_palette_pixels(
            out_w as u16,
            out_h as u16,
            &*indices,
            &*palette_bytes,
            None,
        );
        frame.delay = delay;

        encoder
            .write_frame(&frame)
            .map_err(|e| AppError::Export(e.to_string()))?;

        on_progress(logical + 1);
    }
```

Do the same for `export_video` — change the signature:

```rust
pub fn export_video(
    source: &mut dyn FrameSource,
    layers: &[Layer],
    settings: &ExportSettings,
    output_path: &Path,
    frame_indices: &[usize],
    delays: &[u16],
    on_progress: impl Fn(usize),
) -> Result<(), AppError> {
```

Replace the frame count and fps calculation:

```rust
    let frame_count = frame_indices.len();
    let (src_w, src_h) = source.dimensions();
    let (out_w, out_h) = settings.resize.unwrap_or((src_w, src_h));

    let avg_delay_cs: f64 = if frame_count == 0 {
        10.0
    } else {
        delays.iter().map(|&d| d as f64).sum::<f64>() / frame_count as f64
    };
    let fps = 100.0 / avg_delay_cs.max(1.0);
```

Replace the frame loop:

```rust
    for (logical, &src_i) in frame_indices.iter().enumerate() {
        let base = source.get_frame(src_i)?;
        let composited = compositor::composite_frame(&base, layers, logical);

        let final_img = if (out_w, out_h) != (src_w, src_h) {
            imageops::resize(&composited, out_w, out_h, imageops::FilterType::Lanczos3)
        } else {
            composited
        };

        let png_path = temp_dir.path().join(format!("frame_{logical:06}.png"));
        final_img
            .save(&png_path)
            .map_err(|e| AppError::Export(e.to_string()))?;

        on_progress(logical + 1);
    }
```

- [ ] **Step 2: Update export_project command to pass frame indices**

In `src-tauri/src/commands.rs`, update the `export_project` command to build the visible frame index list:

```rust
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

    // Build visible frame index list and delays.
    let frame_indices: Vec<usize> = (0..project.visible_frame_count())
        .filter_map(|li| project.logical_to_source(li))
        .collect();
    let delays = project.visible_delays();

    let on_progress = |frames_done: usize| {
        let _ = app.emit("export-progress", frames_done);
    };

    match settings.format {
        export::ExportFormat::Gif => {
            export::export_gif(
                project.source.as_mut(),
                &layers,
                &settings,
                out,
                &frame_indices,
                &delays,
                on_progress,
            )
        }
        export::ExportFormat::Mp4 | export::ExportFormat::WebM => {
            export::export_video(
                project.source.as_mut(),
                &layers,
                &settings,
                out,
                &frame_indices,
                &delays,
                on_progress,
            )
        }
    }
}
```

- [ ] **Step 3: Fix export tests**

The export tests in `src-tauri/tests/export_test.rs` call `export_gif` directly. They need to pass the new `frame_indices` and `delays` parameters. Read the test file and update each call to pass:

```rust
let frame_count = gif.frame_count();
let frame_indices: Vec<usize> = (0..frame_count).collect();
let delays: Vec<u16> = gif.delays().to_vec();
```

Then pass `&frame_indices, &delays` as additional arguments.

- [ ] **Step 4: Run tests**

Run: `cargo test -v 2>&1 | tail -30`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```
feat(export): export only visible frames using index mapping

export_gif and export_video now accept explicit frame index and
delay lists instead of iterating the raw source. The export_project
command builds these lists from the project's visible frame set,
skipping excluded frames. Existing tests updated for new signatures.
```

---

### Task 4: Frontend Commands and Store

**Files:**
- Modify: `src/lib/commands.ts`
- Modify: `src/lib/stores/project.svelte.ts`

- [ ] **Step 1: Add command wrappers**

In `src/lib/commands.ts`, add:

```typescript
export async function deleteFrames(indices: number[]): Promise<GifMetadata> {
  return invoke('delete_frames', { indices });
}

export async function restoreFrames(sourceIndices: number[]): Promise<GifMetadata> {
  return invoke('restore_frames', { sourceIndices });
}

export async function getExcludedFrames(): Promise<number[]> {
  return invoke('get_excluded_frames');
}
```

- [ ] **Step 2: Add store methods**

In `src/lib/stores/project.svelte.ts`, add a new field and methods to `ProjectStore`:

```typescript
  excludedFrames = $state<number[]>([]);

  async deleteFrames(logicalIndices: number[]) {
    this.metadata = await cmd.deleteFrames(logicalIndices);
    this.excludedFrames = await cmd.getExcludedFrames();
    this.layers = await cmd.getLayers();
    this.framePaths = new Map(); // clear cache — indices shifted
  }

  async restoreFrames(sourceIndices: number[]) {
    this.metadata = await cmd.restoreFrames(sourceIndices);
    this.excludedFrames = await cmd.getExcludedFrames();
    this.layers = await cmd.getLayers();
    this.framePaths = new Map();
  }

  async restoreAllFrames() {
    if (this.excludedFrames.length === 0) return;
    await this.restoreFrames([...this.excludedFrames]);
  }
```

Also update the `open` method to reset excluded state:

```typescript
  async open(path: string) {
    this.metadata = await cmd.openFile(path);
    this.layers = [];
    this.framePaths = new Map();
    this.excludedFrames = [];
  }
```

- [ ] **Step 3: Commit**

```
feat(frontend): add deleteFrames, restoreFrames commands and store methods
```

---

### Task 5: Timeline Multi-Select and Delete/Restore UI

**Files:**
- Modify: `src/lib/components/Timeline.svelte`

- [ ] **Step 1: Add selection state**

In the `<script>` section of `Timeline.svelte`, add after the existing state declarations:

```typescript
  // Frame selection state
  let selectedFrames = $state<Set<number>>(new Set());
  let lastSelectedFrame = $state<number | null>(null);

  function toggleFrameSelection(index: number, e: MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      // Toggle individual frame
      const next = new Set(selectedFrames);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      selectedFrames = next;
      lastSelectedFrame = index;
    } else if (e.shiftKey && lastSelectedFrame !== null) {
      // Range select
      const lo = Math.min(lastSelectedFrame, index);
      const hi = Math.max(lastSelectedFrame, index);
      const next = new Set(selectedFrames);
      for (let i = lo; i <= hi; i++) {
        next.add(i);
      }
      selectedFrames = next;
    } else {
      // Normal click — navigate, clear selection
      ui.setFrame(index);
      selectedFrames = new Set();
      lastSelectedFrame = null;
    }
  }

  async function handleDeleteSelected() {
    if (selectedFrames.size === 0) return;
    const indices = Array.from(selectedFrames).sort((a, b) => a - b);
    try {
      await project.deleteFrames(indices);
      // Snap current frame if needed
      if (project.metadata && ui.currentFrame >= project.metadata.frame_count) {
        ui.setFrame(Math.max(0, project.metadata.frame_count - 1));
      }
    } catch (e) {
      console.error('Delete frames failed:', e);
    }
    selectedFrames = new Set();
    lastSelectedFrame = null;
  }

  async function handleKeepSelected() {
    if (selectedFrames.size === 0 || !project.metadata) return;
    // Delete everything NOT selected
    const toDelete: number[] = [];
    for (let i = 0; i < project.metadata.frame_count; i++) {
      if (!selectedFrames.has(i)) {
        toDelete.push(i);
      }
    }
    if (toDelete.length === 0) return;
    try {
      await project.deleteFrames(toDelete);
      if (project.metadata && ui.currentFrame >= project.metadata.frame_count) {
        ui.setFrame(Math.max(0, project.metadata.frame_count - 1));
      }
    } catch (e) {
      console.error('Keep selected failed:', e);
    }
    selectedFrames = new Set();
    lastSelectedFrame = null;
  }

  async function handleRestoreAll() {
    try {
      await project.restoreAllFrames();
    } catch (e) {
      console.error('Restore failed:', e);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Delete' && selectedFrames.size > 0) {
      e.preventDefault();
      handleDeleteSelected();
    }
  }
```

- [ ] **Step 2: Add delete/restore buttons to controls row**

In the controls row (after the speed selector `</label>` and before `</div>`), add:

```svelte
      {#if selectedFrames.size > 0}
        <button onclick={handleDeleteSelected}
          class="rounded bg-red-600 px-2 py-0.5 text-xs font-medium text-white hover:bg-red-500"
          title="Delete selected frames">
          Delete ({selectedFrames.size})
        </button>
        <button onclick={handleKeepSelected}
          class="rounded bg-amber-600 px-2 py-0.5 text-xs font-medium text-white hover:bg-amber-500"
          title="Keep only selected frames">
          Keep ({selectedFrames.size})
        </button>
      {/if}
      {#if project.excludedFrames.length > 0}
        <button onclick={handleRestoreAll}
          class="rounded bg-zinc-600 px-2 py-0.5 text-xs font-medium text-white hover:bg-zinc-500"
          title="Restore all deleted frames">
          Restore ({project.excludedFrames.length})
        </button>
      {/if}
```

- [ ] **Step 3: Update thumbnail click handler and add selection visual**

Replace the thumbnail `<div>` in the `{#each}` block:

```svelte
        {#each thumbnails as src, i}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="relative h-12 w-16 shrink-0 cursor-pointer overflow-hidden rounded border-2
              {i === ui.currentFrame ? 'border-blue-400' : selectedFrames.has(i) ? 'border-amber-400' : 'border-zinc-600'}"
            onclick={(e) => toggleFrameSelection(i, e)}
          >
            {#if src}
              <img {src} alt="Frame {i + 1}" class="h-full w-full object-cover" />
            {:else}
              <div class="h-full w-full bg-zinc-700"></div>
            {/if}
            {#if selectedFrames.has(i)}
              <div class="absolute inset-0 bg-amber-400/20"></div>
            {/if}
            <span class="absolute bottom-0 left-0 right-0 bg-black/50 text-center text-[9px] leading-3 text-zinc-300">
              {i + 1}
            </span>
          </div>
        {/each}
```

- [ ] **Step 4: Add keydown listener**

On the outermost `<div>` of the timeline (the one with `class="flex h-full flex-col"`), add:

```svelte
  <div class="flex h-full flex-col" onkeydown={handleKeydown} tabindex="-1">
```

The `tabindex="-1"` makes it focusable so it receives keyboard events.

- [ ] **Step 5: Clear selection when metadata changes**

In the existing thumbnail-loading `$effect`, add at the top after `thumbnails = new Array(count).fill('')`:

```typescript
    selectedFrames = new Set();
    lastSelectedFrame = null;
```

- [ ] **Step 6: Run checks**

Run: `npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep -c ERROR`

Expected: 3 (pre-existing).

Run: `cargo test 2>&1 | grep "test result"`

Expected: all pass.

- [ ] **Step 7: Commit**

```
feat(timeline): add multi-select frame deletion and restore UI

Ctrl+click toggles individual frame selection, Shift+click selects
a range. "Delete" and "Keep" buttons appear when frames are selected.
Delete key shortcut works when frames are selected. A "Restore"
button appears when frames have been excluded, restoring all at once.
Selected frames show an amber highlight. Frame cache and selection
clear when the frame set changes.
```

---

### Task 6: Verify and Final Commit

- [ ] **Step 1: Run full test suite**

Run: `cargo test 2>&1 | grep "test result"`

Expected: all test suites pass.

- [ ] **Step 2: Run svelte-check**

Run: `npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep -c ERROR`

Expected: 3 (pre-existing).

- [ ] **Step 3: Manual smoke test**

Run: `pnpm tauri dev`

Test:
1. Open a GIF with 3+ frames
2. Ctrl+click frames 1 and 3 — verify amber highlight
3. Click "Delete (2)" — verify frames disappear, timeline shows remaining frames
4. Click "Restore (2)" — verify frames reappear
5. Shift+click frame 0, then Shift+click last frame — verify all selected
6. Click "Keep" with only first 2 selected — verify others deleted
7. Add a text layer, set frame range, delete a frame before the range — verify range adjusts
8. Export — verify output only contains visible frames

- [ ] **Step 4: Final commit**

```
feat: add soft-delete frame elimination with multi-select and restore

Frames can be deleted individually (Ctrl+click) or by range
(Shift+click), with "Delete Selected" and "Keep Selected" actions.
Deleted frames are soft-deleted and restorable via the "Restore"
button. Layer frame ranges auto-adjust when frames are excluded or
restored to maintain alignment with source content.

The export pipeline only outputs visible frames, with correct timing
from the visible delays. At least one frame must remain visible.
```
