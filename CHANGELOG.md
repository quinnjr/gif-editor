# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Layer-based GIF editing with image and text overlays
- GIF decoder with lazy frame decoding and LRU cache (cap 50 frames)
- Per-pixel alpha compositing with scale, opacity, and frame range filtering
- Text rasterization with configurable font size, color, and stroke outlines
- Bundled LiberationSans-Bold font for meme-style text
- Export to GIF with high-quality color quantization (imagequant)
- Export to MP4/WebM via ffmpeg subprocess (conditionally enabled)
- Tauri v2 desktop shell with Svelte 5 + Tailwind CSS 4 frontend
- Canvas-based real-time preview with client-side compositing
- Drag-to-position for image and text overlays on the canvas
- Layer panel with visibility toggle, opacity slider, and removal
- Frame timeline with thumbnail strip, playback controls, and speed selector
- Draggable frame range handles on the timeline per layer
- Export dialog with format selection, quality slider, and progress bar
- Toast notifications for errors
- Preview Export toggle for server-rendered composite verification
- Backend integration tests (19 tests) and frontend test infrastructure (3 tests)
