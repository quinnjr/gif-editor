# Frame Deletion

**Date:** 2026-04-04
**Status:** Approved

## Overview

Add the ability to delete (hide) and restore frames from the project's
source media. Supports individual frame selection, multi-select, range
selection, and range trim. Deleted frames are soft-deleted — hidden from
the timeline, playback, and export but restorable without re-opening the
file. Layer frame ranges auto-adjust to maintain visual alignment with
the same source content.

## Data Model

Add to `Project`:

```rust
pub excluded_frames: BTreeSet<usize>,  // source indices that are hidden
```

Helper methods on `Project`:

- `logical_to_source(logical: usize) -> Option<usize>` — maps a
  displayed frame index to the underlying source index by skipping
  entries in `excluded_frames`. Returns `None` if out of bounds.
- `source_to_logical(source: usize) -> Option<usize>` — inverse
  mapping. Returns `None` if the source frame is excluded.
- `visible_frame_count() -> usize` — returns
  `source.frame_count() - excluded_frames.len()`.
- `visible_delays() -> Vec<u16>` — returns delays for non-excluded
  frames only, preserving order.

`GifMetadata` sent to the frontend uses `visible_frame_count()` and
`visible_delays()` so the frontend never sees excluded frames.

All frame-accessing methods (`get_frame_png_path`, `render_composite`)
translate logical indices to source indices via `logical_to_source`
before hitting the `FrameSource`.

## Backend Commands

### `delete_frames(indices: Vec<usize>)`

Accepts logical frame indices. For each index:

1. Map to source index via `logical_to_source`.
2. Add to `excluded_frames`.

After all additions:

3. Auto-adjust layer `frame_range` values: convert each layer's current
   range endpoints from logical to source (before exclusion), then
   recompute via `source_to_logical` after the new exclusions. If a
   layer's entire range is excluded, clamp to `(0, 0)`.
4. If deleting would remove all frames, reject the operation (at least
   one frame must remain).
5. If `currentFrame` (tracked frontend-side) would point beyond the new
   visible count, the frontend snaps to the last visible frame.
6. Return refreshed `GifMetadata`.

### `restore_frames(source_indices: Vec<usize>)`

Accepts source indices. Removes them from `excluded_frames`, re-adjusts
layer ranges via the same source↔logical recomputation, returns
refreshed `GifMetadata`.

### `get_excluded_frames()`

Returns `Vec<usize>` of excluded source indices so the frontend can
populate the restore UI.

## Layer Frame Range Auto-Adjustment

When frames are excluded or restored:

1. Before modifying `excluded_frames`, convert each layer's
   `frame_range` endpoints from current-logical to source indices.
2. Modify `excluded_frames`.
3. Convert each layer's source-index endpoints back to new-logical
   indices via `source_to_logical`.
4. If an endpoint's source frame is now excluded, find the nearest
   non-excluded source frame in the appropriate direction (inward for
   the affected endpoint).
5. If the entire range is excluded, set `frame_range = (0, 0)`.

This preserves the user's intent: a layer that was visible on specific
source frames stays visible on those same frames after other frames are
deleted around it.

## Timeline UI

### Frame Selection

- **Click** a thumbnail: navigate to that frame (existing behavior).
- **Ctrl+click**: toggle frame selection without navigating.
- **Shift+click**: select range from last-selected to clicked frame.
- Visual indicator: selected frames get a blue tinted overlay,
  distinct from the current-frame highlight (blue border).

### Delete Actions

Appear in the timeline controls bar when frames are selected:

- **"Delete Selected"** button + Delete key shortcut: hides the
  selected frames.
- **"Keep Selected"** button: hides everything NOT selected (range
  trim).
- After either action: clear selection, refresh metadata and
  thumbnails, snap current frame if needed.

### Restore UI

A **"Restore Frames"** button in the controls bar, visible when
`excluded_frames` is non-empty:

- Opens a small popover listing excluded frames as thumbnails with
  their original source index.
- Click a thumbnail to restore that single frame.
- **"Restore All"** button at the top of the popover restores
  everything.
- After restore: refresh metadata and thumbnails.

### Guards

- Prevent deleting all frames: if the user's selection covers every
  visible frame, disable the delete action and show a tooltip
  explaining at least one frame must remain.

## Export Changes

- `export_gif` and `export_video` iterate `0..visible_frame_count()`,
  mapping each logical index to a source index via `logical_to_source`.
- Delays use `visible_delays()`.
- Progress reporting uses `visible_frame_count()` as the total.
- No changes to ffmpeg invocation, codec selection, or audio
  passthrough.

## Scope Boundaries

**In scope:**
- Soft-delete individual frames (Ctrl+click multi-select)
- Shift+click range selection
- Delete Selected / Keep Selected actions
- Restore individual or all excluded frames
- Layer frame_range auto-adjustment on delete and restore
- Export only visible frames
- At-least-one-frame guard

**Out of scope:**
- Drag-to-reorder frames
- Frame duplication
- Undo/redo stack (restore covers the undo case for deletion)
- Persistent excluded-frames state across sessions (resets on reopen)
