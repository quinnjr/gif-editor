# Agents

Instructions for AI agents and automated tools working on this codebase.

## Project Overview

Desktop media editor built with Tauri v2 (Rust backend) and Svelte 5 + Tailwind CSS 4 (frontend). Layer-based compositing model for adding image/text overlays with affine transforms, keyframe animation, and frame management to GIFs, videos, and static images.

## Build & Test Commands

```bash
# Install frontend dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production (deb, rpm, AppImage)
pnpm tauri:build

# Build Flatpak
flatpak-builder --user --install --force-clean flatpak-build dev.quinnjr.gif-editor.yml

# Backend tests
cargo test

# Frontend tests
pnpm vitest run

# Coverage reports
cargo tarpaulin --skip-clean --out stdout -- --test-threads=1
pnpm vitest run --coverage

# Lint
pnpm lint
cargo clippy
cargo fmt --check
```

## Code Layout

```
src-tauri/src/
  commands.rs         Tauri IPC command handlers (async, lock ProjectState)
  compositor.rs       Affine warp compositing with bilinear interpolation
  error.rs            AppError enum with Serialize for Tauri transport
  export.rs           GIF/MP4/WebM/still export with frame index mapping + audio passthrough
  flare_renderer.rs   Procedural lens flare rendering (additive compositing)
  font_data.rs        Serves embedded TTF bytes to the WebView preview over IPC
  fonts.rs            Font loading (bundled Anton + LiberationSans-Bold)
  frame_source.rs     FrameSource trait (GIF, video, static image)
  gif_decoder.rs      GIF decoding with disposal handling + LRU frame cache
  image_source.rs     Static image source with expandable timeline
  layer.rs            Layer model, Keyframe struct, interpolation function
  lib.rs              Tauri app bootstrap
  project.rs          Project state, frame exclusion, layer CRUD, index mapping
  text_renderer.rs    Text rasterization via ab_glyph
  video_decoder.rs    MP4/WebM decoding via ffmpeg/ffprobe subprocesses

src/lib/
  commands.ts         Typed invoke() wrappers for all Tauri commands
  types.ts            TypeScript interfaces mirroring Rust data model
  components/
    Canvas.svelte     Preview canvas with affine handles + keyframe drag
    ExportDialog.svelte  Export modal with progress bar
    LayerItem.svelte  Layer properties (opacity slider, keyframe-aware)
    LayerPanel.svelte Layer list
    Timeline.svelte   Thumbnails, playback, frame selection, keyframe markers
    Toast.svelte      Error notifications
    Toolbar.svelte    File open, layer creation, export trigger
  stores/
    project.svelte.ts Reactive project state with frame deletion/restore
    ui.svelte.ts      UI state (selection, playback, preview mode)
  utils/
    canvas-renderer.ts  Client-side compositing with affine transforms + keyframe interpolation

src-tauri/tests/      Integration tests (roughly one file per source module)
src/tests/            Frontend unit tests (Vitest + jsdom)
```

## Key Conventions

### Rust

- **Crate:** `gif-editor`, **lib:** `gif_editor_lib` — tests import from `gif_editor_lib`
- **Edition:** 2024
- All IPC types derive `Serialize` (and `Deserialize` where received from frontend)
- `AppError` is the single error type — serializes as a string for Tauri
- `image_data` and `frames` fields on layers are `#[serde(skip)]` — pixel buffers never cross IPC
- Project state is `Mutex<Option<Project>>` — all commands are `async` and lock/unwrap
- `cargo clippy` must pass with zero warnings
- `cargo fmt` must be applied before committing

### Frontend

- **Svelte 5 runes only** — `$state`, `$effect`, `$props`, `$derived`. No legacy reactivity.
- **Tailwind CSS 4** — utility classes, dark theme (zinc palette)
- Use `SvelteSet` (from `svelte/reactivity`) instead of `Set` for reactive collections
- `{@const}` must be direct child of block tags (`{#if}`, `{#each}`), not elements
- Stores are class-based singletons from `.svelte.ts` files
- All Tauri calls go through `src/lib/commands.ts`, never raw `invoke()`
- ESLint must pass: `pnpm lint`

### Git

- **Git-flow branching:** `main` (stable), `develop` (integration), `feature/*`, `release/*`, `hotfix/*`
- **Conventional Commits:** `<type>(<scope>): <subject>` with mandatory body for non-trivial changes
- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`
- Never commit plan/design docs (`docs/plans/`) or coverage reports (`coverage/`)

### Testing

- Backend: integration tests in `src-tauri/tests/`, roughly one test file per source module; trait and bootstrap modules (`frame_source.rs`, `lib.rs`, `main.rs`) are covered via their implementors
- Tauri command handlers in `commands.rs` are thin lock-and-delegate wrappers; they are tested through the `AppState`/`Project` methods they delegate to (e.g. `undo_test.rs`, `project_test.rs`) rather than through `tauri::State`
- Fixtures: `src-tauri/tests/fixtures/` (test.gif, test.mp4, test.png)
- Frontend: `src/tests/` with Vitest + jsdom, Tauri API mocked via `vi.mock`
- Run both `cargo test` and `pnpm vitest run` before committing

## Architecture Notes

- **Backend is source of truth** — frontend holds lightweight copies for fast preview
- **Two rendering paths:** client-side Canvas preview (instant) and server-side compositor (pixel-accurate export)
- **Affine transforms** use `ctx.transform()` on frontend and inverse-mapping with bilinear interpolation on backend
- **Keyframe animation** — `interpolate_keyframes()` exists in both Rust and TypeScript with identical logic
- **Frame exclusion** — `excluded_frames: BTreeSet<usize>` with logical↔source index mapping; layers and keyframes auto-remap on delete/restore
- **GIF/video frames are lazily decoded** — LRU cache (cap 50) bounds memory
- **ImageSource** supports timeline expansion when animated GIF overlays are added to static images
- **Video decode/encode** shells out to `ffmpeg`/`ffprobe` (optional, checked at runtime)
- **Asset protocol** must be enabled with `requireLiteralLeadingDot: false` for temp dir access (Rust `tempfile` creates `.tmp`-prefixed dirs)
- **Tauri commands are async** to avoid blocking the main thread (critical for WebKitGTK asset serving)
