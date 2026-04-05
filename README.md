# GIF Editor

A desktop application for editing animated GIFs, videos, and images with a layer-based compositing model. Open a GIF, MP4, WebM, or static image, add animated GIF overlays and text with affine transforms and keyframe animation, manage frames, preview in real time, and export to GIF, MP4, or WebM.

Built with [Tauri v2](https://tauri.app/) (Rust backend) and [Svelte 5](https://svelte.dev/) + [Tailwind CSS 4](https://tailwindcss.com/) (frontend).

## Features

- **Multi-format input** — open GIF, MP4, WebM, PNG, JPG, or WebP as the base project
- **Animated GIF overlays** — add animated GIFs as layers that loop in sync with the timeline
- **Layer-based editing** — image and text overlays with per-frame control via draggable frame ranges
- **Affine transforms** — non-uniform scale and skew via interactive canvas drag handles (corners for scale, edges for skew, Shift for free-form)
- **Keyframe animation** — animate position and opacity across frames with linear interpolation; drag on any frame to set a keyframe
- **Frame management** — soft-delete frames with multi-select (Ctrl+click, Shift+click), Keep/Delete/Restore actions
- **Real-time preview** — client-side Canvas compositing with full affine transform support
- **Text with stroke outlines** — configurable font, size, color, and meme-style stroke
- **Export to GIF, MP4, or WebM** — GIF uses imagequant quantization; video uses ffmpeg with audio preservation
- **Dark mode** — native dark theme for all UI elements including dropdown menus

## Prerequisites

- [Rust](https://rustup.rs/) (stable or nightly)
- [Node.js](https://nodejs.org/) >= 20
- [pnpm](https://pnpm.io/) >= 10
- System dependencies for Tauri on Linux: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev` (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))
- **Optional:** [ffmpeg](https://ffmpeg.org/) on PATH for video import/export (GIF-only editing works without it)

## Getting Started

```bash
# Install frontend dependencies
pnpm install

# Run in development mode (opens the app with hot reload)
pnpm tauri dev

# Build for production (deb, rpm, AppImage)
pnpm tauri:build

# Lint
pnpm lint
```

## Installation

### AppImage / deb / rpm

```bash
pnpm tauri:build
# Bundles are in target/release/bundle/
```

### Flatpak

```bash
flatpak-builder --user --install --force-clean flatpak-build dev.quinnjr.gif-editor.yml
flatpak run dev.quinnjr.gif-editor
```

## Project Structure

```
src-tauri/              Rust backend
  src/
    commands.rs         Tauri IPC command handlers
    compositor.rs       Affine warp compositing with bilinear interpolation
    error.rs            Typed error enum
    export.rs           GIF/MP4/WebM export pipeline with audio passthrough
    fonts.rs            Font loading (bundled + system)
    frame_source.rs     FrameSource trait (GIF, video, static image)
    gif_decoder.rs      GIF decoding with LRU frame cache
    image_source.rs     Static image source with expandable timeline
    layer.rs            Layer model, keyframes, affine interpolation
    lib.rs              App setup, module registration
    project.rs          Project state, frame exclusion, index mapping
    text_renderer.rs    Text rasterization via ab_glyph
    video_decoder.rs    MP4/WebM decoding via ffmpeg subprocess
  fonts/                Bundled font (LiberationSans-Bold)
  tests/                42 backend integration tests

src/                    Svelte 5 frontend
  lib/
    components/
      Canvas.svelte     Preview canvas with drag handles and keyframe creation
      ExportDialog.svelte  Export settings modal with progress bar
      LayerItem.svelte  Layer properties (opacity, keyframe-aware)
      LayerPanel.svelte Layer list panel
      Timeline.svelte   Thumbnails, playback, frame selection, keyframe markers
      Toast.svelte      Error notifications
      Toolbar.svelte    File open, layer creation, export trigger
    stores/
      project.svelte.ts Reactive project state with frame deletion
      ui.svelte.ts      UI state (selection, playback, preview)
    utils/
      canvas-renderer.ts  Client-side compositing with affine transforms
    commands.ts         Typed Tauri invoke() wrappers
    types.ts            TypeScript interfaces (LayerInfo, Keyframe, etc.)
  routes/
    +page.svelte        Main app layout
  tests/                Frontend tests (Vitest + jsdom)
```

## Architecture

The app uses a two-process architecture:

- **Rust backend** — source of truth for all project state. Handles frame decoding (GIF, video, static image), affine warp compositing with bilinear interpolation, text rasterization, keyframe interpolation, frame exclusion mapping, and export encoding.
- **Svelte frontend** — renders a Canvas-based preview using the Canvas 2D `transform()` API for instant feedback. Interpolates keyframes client-side to match the backend. Final export always goes through the Rust backend.

Communication happens via async Tauri commands (frontend calls Rust) and events (backend pushes export progress).

## Testing

```bash
# Run backend tests (42 tests)
cargo test

# Run frontend tests
pnpm vitest run

# Type-check the frontend
pnpm check

# Lint
pnpm lint
```

## License

[MIT](LICENSE.md)
