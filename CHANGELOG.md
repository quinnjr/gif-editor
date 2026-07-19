# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Snapshot-based undo/redo (50-entry history) with Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y shortcuts and toolbar buttons
- Rotation for all layer types, applied in both preview and export compositors, with rotation input, ±90° buttons, and flip H/V controls
- Solar flare layers: procedural lens flare renderer (glow, starburst, ghosts, halo ring) with intensity, scale, and pulse-speed controls, composited additively
- Text word wrap with max width, horizontal alignment (left/center/right), meme text preset, and bundled Anton (Impact-style) font
- Still-image export: PNG, JPEG, and WebP single-frame export with frame selection
- Layer duplication and layer reorder (move up/down) controls in the layer panel
- Timeline drag-to-pan scrolling on the frame strip
- Resize all layers uniformly: Ctrl/Cmd+Shift+= / Ctrl/Cmd+Shift+- shortcuts and Alt while dragging canvas handles
- GIF frame-disposal handling (Keep/Background/Previous) so delta-optimized GIFs decode correctly
- Husky + commitlint for Conventional Commits enforcement

### Changed

- Performance: text layers rasterize once per content change instead of once per frame (LRU render caches on both the export and preview paths); bundled fonts parse once per process; flare rendering allocates only the lit bounding region; video export streams all frames from a single ffmpeg process instead of one seek+decode process per frame; temp preview/export PNGs use fast compression
- Memory: layer pixel buffers are Arc-shared, so undo/redo snapshots and layer duplication copy pointers instead of pixel data; the preview's decoded-frame cache is bounded (LRU)
- Export no longer holds the project state lock for its duration — layer edits and undo/redo keep working while an export encodes; fetching an uncached frame during export reports "export in progress"
- All ffmpeg/ffprobe invocations are bounded by timeouts; a wedged subprocess can no longer hang the app
- Release builds enable LTO, single codegen unit, symbol stripping, and size-oriented optimization
- Bundled fonts ship once (embedded in the binary; the preview loads them over IPC via the FontFace API instead of a duplicate static copy)

### Fixed

- Panic (and permanently poisoned app state) when adding a text layer whose content hit a multi-byte UTF-8 boundary at the name-truncation point
- Multi-minute UI hang when a flare layer was positioned far off-canvas (integer wrap in render bounds)
- Still-image export used the wrong source frame when frames had been deleted
- Setting max width or stroke to empty/null now actually clears them instead of being silently ignored
- Alt-drag resize-all compounded scale exponentially per pointer move; now applies the gesture ratio absolutely and records one history entry per gesture
- Error toasts after the first one never displayed; errors from canvas rendering, timeline actions, and slider updates were silently dropped
- Undo/redo after frame deletion desynced the timeline (excluded frames and thumbnails now refresh)
- Shift+click frame range selection now works from a plain-click anchor
- Selection outline, drag handles, and hit-testing now account for layer rotation
- Preview↔export parity: text font size, line height, typeface, stroke-pad anchoring, and opacity flattening now match between the canvas preview and the export compositor; flare colors and brightness constants aligned
- Slider drags and frame-range drags no longer flood the undo history (one entry per gesture)
- Preview text now renders with the same bundled fonts the exporter uses (Anton, Liberation Sans)

## [0.1.0] - 2026-04-05

### Added

- **Core editing**
  - Layer-based editing with image and text overlays
  - GIF decoder with lazy frame decoding and LRU cache
  - Per-pixel alpha compositing with affine transforms
  - Text rasterization with configurable font, color, and stroke

- **Video support**
  - Open and edit MP4/WebM files via ffmpeg subprocess decoding
  - FrameSource abstraction over GIF, video, and static image sources
  - Open static images (PNG, JPG, WebP) as single-frame projects
  - Audio preservation on video re-export (AAC for MP4, Opus for WebM)

- **Animated GIF overlays**
  - Add animated GIFs as overlay layers (frame-synced, looping)
  - Timeline auto-expands when GIF overlay has more frames than base

- **Affine transforms**
  - Non-uniform scale (X/Y) and skew (X/Y) for all layer types
  - Interactive canvas drag handles: corners for scale, edges for skew
  - Shift modifier for free-form vs proportional corner scaling
  - Bilinear interpolation in backend compositor

- **Keyframe animation**
  - Keyframe-based position and opacity animation with linear interpolation
  - Drag overlay on any frame to create/update a keyframe
  - First-keyframe seeding (auto-creates frame 0 anchor)
  - Yellow diamond markers on timeline for keyframe positions
  - Right-click timeline diamonds to delete keyframes
  - Opacity slider respects and creates keyframes

- **Frame management**
  - Soft-delete frames with Ctrl+click multi-select and Shift+click range
  - "Delete Selected" and "Keep Selected" actions
  - Restore deleted frames (frames are hidden, not destroyed)
  - Layer frame ranges auto-adjust on delete/restore
  - Keyframe indices remap on frame deletion

- **Export**
  - GIF export with imagequant color quantization
  - MP4 export via ffmpeg (libx264, AAC audio)
  - WebM export via ffmpeg (libvpx-vp9, Opus audio)
  - Export respects frame deletions (only visible frames)
  - Progress bar with frame counter

- **UI/UX**
  - Tauri v2 desktop app with Svelte 5 + Tailwind CSS 4
  - Canvas preview with client-side compositing
  - Layer panel with visibility, opacity, frame range display
  - Frame timeline with thumbnails, playback, speed control
  - Export dialog with format selection and quality slider
  - Dark mode dropdown styling for WebKitGTK
  - Sequential thumbnail loading to prevent UI freezing
  - Async Tauri commands to keep main thread responsive
  - Delete key shortcut for frame deletion
  - Toast notifications for errors

- **Build & distribution**
  - Flatpak manifest (GNOME 48 SDK) with ffmpeg, x264, libvpx, opus
  - AppStream metainfo and desktop entry with MIME associations
  - AppImage, deb, and rpm bundle support via `pnpm tauri:build`
  - GitHub Actions release workflow for Linux/macOS/Windows on tag push
  - ESLint with Svelte and TypeScript rules

- **Testing & quality**
  - 102 Rust integration tests (90.75% source coverage)
  - 77 TypeScript unit tests (100% coverage)
  - cargo clippy clean (zero warnings)
  - cargo fmt applied

[Unreleased]: https://github.com/quinnjr/gif-editor/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/quinnjr/gif-editor/releases/tag/v0.1.0
