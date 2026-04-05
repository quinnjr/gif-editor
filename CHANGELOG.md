# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
