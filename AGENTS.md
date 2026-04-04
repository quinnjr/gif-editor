# Agents

Instructions for AI agents and automated tools working on this codebase.

## Project Overview

Desktop GIF editor built with Tauri v2 (Rust backend) and Svelte 5 + Tailwind CSS 4 (frontend). Layer-based compositing model for adding image/text overlays to animated GIFs.

## Build & Test Commands

```bash
# Install frontend dependencies
pnpm install

# Build everything (frontend + backend)
pnpm tauri build

# Run backend tests (Rust, 19 tests)
cargo test

# Run frontend tests (Vitest, 3 tests)
pnpm vitest run

# Type-check frontend
pnpm exec svelte-check

# Run the app in development mode
pnpm tauri dev
```

## Code Layout

- `src-tauri/src/` — Rust backend modules
- `src/lib/components/` — Svelte 5 UI components
- `src/lib/stores/` — Svelte 5 reactive state (uses `$state` runes)
- `src/lib/commands.ts` — typed Tauri IPC wrappers
- `src/lib/types.ts` — TypeScript interfaces mirroring Rust data model

## Key Conventions

### Rust

- **Crate name:** `gif-editor`, **lib name:** `gif_editor_lib` — tests import from `gif_editor_lib`
- **Edition:** 2024
- All public types that cross the IPC boundary derive `Serialize` (and `Deserialize` where needed from frontend)
- `AppError` is the single error type; implements `Serialize` by stringifying for Tauri transport
- `image_data` fields on layers are `#[serde(skip)]` — pixel buffers never cross IPC
- Project state is `Mutex<Option<Project>>` managed by Tauri — all commands lock and unwrap

### Frontend

- **Svelte 5 runes only** — use `$state`, `$effect`, `$props`, `$derived`. No legacy `let`/`$:` reactivity.
- **Tailwind CSS 4** — utility classes, dark theme (zinc palette), no component library
- Stores are class-based singletons exported from `.svelte.ts` files
- All Tauri calls go through `src/lib/commands.ts`, never raw `invoke()`

### Git

- **Git-flow branching:** `main` (stable), `develop` (integration), `feature/*`, `release/*`, `hotfix/*`
- **Conventional Commits** required: `<type>(<scope>): <subject>` — see CLAUDE.md for full rules
- Commit bodies are mandatory for non-trivial changes — explain why, not what
- Never commit plan/design docs to git

### Testing

- Backend tests live in `src-tauri/tests/` as integration tests
- Test GIF fixture at `src-tauri/tests/fixtures/test.gif` is generated programmatically by tests
- Frontend tests use Vitest + jsdom with Tauri API mocks
- Run `cargo test` and `pnpm vitest run` before committing

## Architecture Notes

- **Backend is source of truth** — frontend holds lightweight copies for fast preview, syncs changes back via commands
- **Canvas preview is client-side** — the frontend composites layers on HTML Canvas for instant feedback
- **Export is server-side** — the Rust backend composites all layers for pixel-accurate output
- **GIF frames are lazily decoded** — LRU cache (cap 50) bounds memory; frames re-decode from source on eviction
- **Text rasterization** uses `ab_glyph` with a bundled font (LiberationSans-Bold)
- **Video export** shells out to `ffmpeg` (optional dependency, checked at runtime)
