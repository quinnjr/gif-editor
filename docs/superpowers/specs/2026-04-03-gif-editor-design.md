# GIF Editor — Design Spec

## Overview

A desktop GUI application for editing animated GIFs with a layer-based compositing model. Users can open a GIF, add image and text overlays that span arbitrary frame ranges, preview the result in real time, and export to GIF, MP4, or WebM.

Target audience: general (non-technical) users. The UX should be intuitive with clear affordances and no jargon.

## Architecture

**Two-process Tauri application:**

- **Rust backend** — GIF/image decoding, frame extraction, layer compositing, text rasterization, export encoding. Source of truth for all project state.
- **Svelte + Tailwind CSS 4 frontend** — UI controls, Canvas-based preview rendering, drag/position interactions.

Communication via Tauri commands (frontend calls Rust) and events (backend pushes to frontend). Frame data served via Tauri's asset protocol to avoid base64 serialization overhead.

**Key principle:** The frontend handles preview and interaction. The backend handles processing and export. The frontend composites layers on Canvas for real-time feedback, but final export always goes through the Rust backend to guarantee output fidelity.

## Data Model

Project state lives in the Rust backend. The frontend receives a serialized view.

```
Project
├── source_gif: GifData
│   ├── source_path: PathBuf     // original file, used for lazy re-decoding
│   ├── frame_cache: LruCache<usize, RgbaImage>  // decoded RGBA frames, LRU evicted, cap ~50
│   ├── frame_count: usize
│   ├── delays: Vec<u16>         // ms delay per frame
│   └── dimensions: (u32, u32)   // width x height
│
├── layers: Vec<Layer>           // ordered back-to-front
│   ├── ImageLayer
│   │   ├── id: Uuid
│   │   ├── name: String
│   │   ├── image_data: RgbaImage // decoded source image
│   │   ├── position: (f64, f64)  // x, y relative to canvas
│   │   ├── scale: f64
│   │   ├── opacity: f64          // 0.0 - 1.0
│   │   ├── frame_range: (usize, usize) // start, end (inclusive)
│   │   └── visible: bool
│   │
│   └── TextLayer
│       ├── id: Uuid
│       ├── name: String
│       ├── text: String
│       ├── font_family: String
│       ├── font_size: f64
│       ├── color: (u8, u8, u8, u8) // RGBA
│       ├── stroke: Option<Stroke>   // outline color: (u8,u8,u8,u8) + width: f64
│       ├── position: (f64, f64)
│       ├── scale: f64
│       ├── opacity: f64          // 0.0 - 1.0
│       ├── frame_range: (usize, usize)
│       └── visible: bool
│
└── export_settings: ExportSettings
    ├── format: ExportFormat      // Gif, Mp4, WebM
    ├── quality: u8
    └── resize: Option<(u32, u32)>
```

Frame range determines which frames a layer appears on. Layer ordering in the Vec determines z-order (last element renders on top).

## UI Layout

```
┌─────────────────────────────────────────────────┐
│  Toolbar                                        │
│  [Open GIF] [Add Image] [Add Text] [Export]     │
├────────────────────────────────┬────────────────┤
│                                │  Layer Panel   │
│                                │                │
│        Canvas Preview          │  [Text: hello] │
│                                │  [Image: hat]  │
│   (drag overlays here)        │  [GIF base]    │
│                                │                │
│                                │  visibility    │
│                                │  opacity slider│
├────────────────────────────────┴────────────────┤
│  Frame Timeline                                 │
│  [1][2][3][4][5][6]...[n]    < >  >>  speed    │
│  ════════════════  frame range drag handles     │
└─────────────────────────────────────────────────┘
```

### Toolbar
File operations (open GIF, export) and adding new layers (image overlay, text overlay).

### Canvas Preview
HTML Canvas showing the current frame with all visible layers composited. Overlays are draggable and resizable directly on the canvas.

Interactions:
- Click a layer's overlay to select it
- Drag to reposition
- Corner handles to scale (maintain aspect ratio by default)
- Double-click a text layer to edit its text inline

### Layer Panel
Lists all layers back-to-front. Each entry shows name, visibility toggle, and opacity slider. Click to select, drag to reorder. Selected layer shows its properties (position, scale, frame range).

### Frame Timeline
Thumbnail strip of all frames. Click to jump to a frame. Playback controls (play/pause, speed). When a layer is selected, its frame range is shown as a draggable bar over the timeline — drag the handles to change which frames the layer appears on.

## Frontend-Backend Communication

### Tauri Commands (frontend calls backend)

| Command | Purpose |
|---|---|
| `open_gif(path)` | Decode GIF, return metadata (frame count, dimensions, delays) |
| `get_frame(index)` | Returns asset URL for a decoded frame PNG |
| `add_image_layer(path)` | Decode image, create layer, return layer ID + metadata |
| `add_text_layer(text, font, size, color, stroke)` | Create text layer, return layer ID |
| `update_layer(id, changes)` | Update position, scale, opacity, frame range, visibility, text content, etc. |
| `reorder_layers(id_list)` | Set new z-order from the layer panel |
| `remove_layer(id)` | Delete a layer |
| `render_composite(frame_index)` | Composite all layers onto a frame server-side (used for export preview toggle in toolbar) |
| `export(settings)` | Run full export pipeline, return file path |
| `get_system_fonts()` | Return list of available font families for text layers |

### Tauri Events (backend pushes to frontend)

| Event | Purpose |
|---|---|
| `export-progress` | Percentage complete during export |

### Preview Strategy

For interactive editing (dragging, resizing), the frontend composites layers on Canvas itself using the raw frame PNGs and overlay images. This is instant and avoids round-tripping to the backend.

A "Preview Export" toggle in the toolbar switches the canvas to server-rendered composites via `render_composite`. This lets the user see exactly what export will produce — particularly useful for verifying text rasterization fidelity before exporting. When toggled off, the canvas returns to fast client-side compositing.

The frontend holds lightweight copies of layer data (positions, sizes, image URLs, text content) to drive the Canvas, and syncs changes back to the backend via `update_layer`.

## Backend Processing

### GIF Decoding
The `gif` crate decodes the source GIF into individual RGBA frames. Frames are decoded lazily — only when first needed for preview or export. Decoded frames are cached in memory with a cap (e.g., 50 frames), evicting least-recently-viewed frames and re-decoding from source on demand. Decoded frames are written as PNGs to a temp directory and served via Tauri's asset protocol.

### Layer Compositing (Export)
For each frame during export:
1. Start with the base GIF frame as the bottom layer
2. For each layer (in z-order) whose frame range includes this frame:
   - **ImageLayer:** scale the source image, then alpha-blend at (x, y) with the given opacity
   - **TextLayer:** rasterize the text (font, size, color, stroke), then alpha-blend at (x, y)
3. Output the composited frame

### Text Rasterization
The `ab_glyph` crate loads fonts and renders text to an RGBA buffer. The app bundles a default meme font (Impact or similar, license permitting) and also supports loading system fonts.

### Export Formats
- **GIF** — re-encode composited frames using the `gif` crate with `imagequant` for high-quality color quantization and dithering
- **MP4/WebM** — shell out to `ffmpeg`, feeding it composited PNG frames. The app checks for `ffmpeg` at startup and disables video export formats if not found, with a message explaining how to install it.

### Key Rust Crates
- `gif` — GIF decode/encode
- `image` — RGBA image manipulation, PNG encode, scaling, alpha compositing
- `ab_glyph` — font loading and text rasterization
- `imagequant` — high-quality color quantization for GIF export
- `tauri` — app shell and IPC

## Error Handling & Edge Cases

**Invalid files:** Corrupt GIFs or unsupported image formats return a typed error; the frontend shows a toast notification.

**Large GIFs:** Lazy frame decoding, capped in-memory frame cache (~50 frames), LRU eviction, loading spinner on timeline for un-cached frames.

**Export failures:**
- `ffmpeg` not found: video export options grayed out with tooltip
- `ffmpeg` crash mid-export: catch error, clean up temp files, show stderr
- Disk full: catch write errors, report clearly

**Undo/Redo:** Not in MVP scope. The layer model supports adding it later (snapshot layer state on mutation, restore on undo). Users can delete/re-add layers as a workaround.

## Testing Strategy

### Rust Backend
- **Unit tests** for core logic: frame decoding (correct dimensions/frame count), layer compositing (pixel-correct blending at given positions/opacities), text rasterization (non-empty output), color quantization
- **Integration tests** for the export pipeline: open a small test GIF, add layers, export to each format, verify output is valid (decodable, correct frame count, correct dimensions)
- Small test GIF fixtures committed to the repo (3-5 frames, < 50KB)

### Frontend
- **Component tests** (Vitest + Svelte testing library) for the layer panel, timeline, and toolbar: adding/removing/reordering layers updates UI correctly
- Canvas interaction covered by manual QA

### E2E
Not in MVP scope. Backend integration tests + frontend component tests cover most risk. E2E testing can be added later.

## Out of Scope (MVP)

- Undo/redo
- Keyframe animation (layer movement/scaling across frames)
- Drawing/painting tools
- Crop/resize
- Frame management (delete, reorder, adjust timing)
- Plugin system
- E2E tests
