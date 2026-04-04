# GIF Editor

A desktop application for editing animated GIFs with a layer-based compositing model. Open a GIF, add image and text overlays that span arbitrary frame ranges, preview in real time, and export to GIF, MP4, or WebM.

Built with [Tauri v2](https://tauri.app/) (Rust backend) and [Svelte 5](https://svelte.dev/) + [Tailwind CSS 4](https://tailwindcss.com/) (frontend).

## Features

- **Layer-based editing** — add image overlays (JPEG, PNG, WebP) and text overlays on top of animated GIFs
- **Per-frame control** — each layer has a frame range controlling which frames it appears on, adjustable via draggable handles on the timeline
- **Real-time preview** — client-side Canvas compositing for instant feedback while dragging and editing
- **Text with meme-style stroke** — configurable font size, color, and outline stroke
- **Export to GIF, MP4, or WebM** — GIF export uses high-quality color quantization; video export uses ffmpeg
- **Drag-to-position** — click and drag overlays directly on the canvas
- **Layer management** — reorder, toggle visibility, adjust opacity, remove layers

## Prerequisites

- [Rust](https://rustup.rs/) (nightly, edition 2024)
- [Node.js](https://nodejs.org/) >= 20
- [pnpm](https://pnpm.io/) >= 10
- System dependencies for Tauri: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- **Optional:** [ffmpeg](https://ffmpeg.org/) for MP4/WebM export (GIF export works without it)

## Getting Started

```bash
# Install frontend dependencies
pnpm install

# Run in development mode (opens the app with hot reload)
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Project Structure

```
src-tauri/              Rust backend
  src/
    commands.rs         Tauri IPC command handlers
    compositor.rs       Layer compositing (alpha blend, scale, opacity)
    error.rs            Typed error enum
    export.rs           GIF/MP4/WebM export pipeline
    fonts.rs            Font loading (bundled + system)
    gif_decoder.rs      GIF decoding with LRU frame cache
    layer.rs            Layer data model (Image, Text, Stroke)
    lib.rs              App setup, module registration
    project.rs          Project state management
    text_renderer.rs    Text rasterization via ab_glyph
  fonts/                Bundled font (LiberationSans-Bold)
  tests/                Backend integration tests

src/                    Svelte frontend
  lib/
    components/
      Canvas.svelte     Preview canvas with drag-to-position
      ExportDialog.svelte  Export settings modal with progress
      LayerItem.svelte  Single layer entry (visibility, opacity)
      LayerPanel.svelte Layer list panel
      Timeline.svelte   Frame thumbnails, playback, frame range
      Toast.svelte      Notifications
      Toolbar.svelte    File operations, layer creation, export
    stores/
      project.svelte.ts Reactive project state
      ui.svelte.ts      UI state (selection, playback, preview)
    utils/
      canvas-renderer.ts  Client-side layer compositing
      drag.ts           Pointer drag interaction helpers
    commands.ts         Typed Tauri invoke() wrappers
    types.ts            TypeScript interfaces
  routes/
    +page.svelte        Main app layout
  tests/                Frontend test infrastructure
```

## Architecture

The app uses a two-process architecture:

- **Rust backend** — source of truth for all project state. Handles GIF decoding, image compositing, text rasterization, and export encoding. Frames are decoded lazily and cached in an LRU cache to bound memory usage.
- **Svelte frontend** — renders a Canvas-based preview by compositing layers client-side for instant feedback. Final export always goes through the Rust backend to guarantee output fidelity.

Communication happens via Tauri commands (frontend calls Rust) and events (backend pushes export progress to frontend).

## Testing

```bash
# Run backend tests (19 tests)
cargo test

# Run frontend tests (3 tests)
pnpm vitest run

# Type-check the frontend
pnpm exec svelte-check
```

## License

MIT
