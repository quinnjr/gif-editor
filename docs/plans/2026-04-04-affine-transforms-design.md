# Affine Transforms for Overlay Layers

**Date:** 2026-04-04
**Status:** Approved

## Overview

Add non-uniform scaling and skew to image and text layers, controlled via
interactive canvas drag handles. Replaces the existing uniform `scale` property
with decomposed `scale_x`, `scale_y`, `skew_x`, `skew_y` fields and updates
the compositor, frontend renderer, and canvas interaction layer accordingly.

## Data Model

Remove `scale: f64` from both `ImageLayer` and `TextLayer`. Add four fields to
each (all `f64`, `Clone`, `Serialize`/`Deserialize` where applicable):

| Field     | Default | Meaning                          |
|-----------|---------|----------------------------------|
| `scale_x` | 1.0     | Horizontal scale factor          |
| `scale_y` | 1.0     | Vertical scale factor            |
| `skew_x`  | 0.0     | Horizontal shear (radians)       |
| `skew_y`  | 0.0     | Vertical shear (radians)         |

The affine matrix built from these plus `position (tx, ty)`:

```
| scale_x   skew_x   tx |
| skew_y    scale_y   ty |
```

Propagate the change through:

- `LayerUpdate` (backend): replace `scale: Option<f64>` with four `Option<f64>` fields
- `LayerInfo` (backend → frontend): replace `scale: f64` with four `f64` fields
- `LayerInfo` TypeScript type: same replacement
- Layer constructors: initialise defaults (scale 1.0, skew 0.0)

## Backend Compositor

Replace the current resize-and-paste approach in `compositor.rs` with affine
warp compositing:

1. Take source image (or rendered text buffer) at original size.
2. Build the affine matrix from `scale_x`, `scale_y`, `skew_x`, `skew_y`,
   `position`.
3. Compute the axis-aligned bounding box of the four transformed corners to
   determine output buffer size and origin offset.
4. Warp the source into the output buffer using bilinear interpolation
   (`imageproc::geometric_transformations::warp_into` or equivalent).
5. Alpha-blend the warped result onto the canvas at the bounding box origin.

**Fast path:** layers with identity transform (both scales 1.0, both skews 0.0)
skip the warp and use the existing `composite_rgba_buffer` for performance.

**New dependency:** `imageproc` crate for affine warp support.

## Frontend Canvas Renderer

Replace the current `ctx.drawImage(img, x, y, w, h)` pattern in
`canvas-renderer.ts` with Canvas 2D affine transforms:

```js
ctx.save();
ctx.globalAlpha = layer.opacity;
ctx.transform(scale_x, skew_y, skew_x, scale_y, tx, ty);
ctx.drawImage(img, 0, 0);   // image layers
// or ctx.fillText(text, 0, 0) for text layers
ctx.restore();
```

This replaces manual width/height calculation for images and scaled font-size
calculation for text. The `ctx.transform()` arguments follow the Canvas 2D spec
column-major order: `(a, b, c, d, e, f)` = `(scale_x, skew_y, skew_x, scale_y, tx, ty)`.

## Canvas Drag Handles

When a layer is selected, render 8 handles over the canvas:

### Handle layout

- **4 corner handles** — for scaling.
  - Default drag: proportional scaling (aspect ratio locked).
  - Shift + drag: free-form independent X/Y scaling.
- **4 edge midpoint handles** — for skewing.
  - Top/bottom edge handles adjust `skew_x`.
  - Left/right edge handles adjust `skew_y`.

### Visual appearance

- 8×8px filled squares at each handle position.
- Thin outline connecting the handles, forming the transformed bounding box
  (parallelogram when skewed).
- Drawn on top of the composited canvas content so handles don't affect the
  rendered output.

### Interaction flow

1. **Pointer down:** hit-test handles first (within a generous grab radius).
   If no handle hit, fall through to the existing layer-body hit test for
   drag-to-move.
2. **Pointer move:** update the relevant properties based on drag delta.
   - Corner handles: compute new `scale_x`/`scale_y` from the ratio of the
     new corner position to the original corner position relative to the
     layer origin. When proportional (no Shift), derive both scales from a
     single diagonal ratio.
   - Edge handles: compute `skew_x` or `skew_y` from the perpendicular
     displacement of the edge midpoint.
   - Update the store directly for instant visual feedback (same pattern as
     existing position drag).
3. **Pointer up:** sync final values to backend via `project.updateLayer()`.

### Hit testing

Update `findLayerAtPoint` in `Canvas.svelte` to account for the affine
transform. Instead of testing the click point against a screen-space AABB,
apply the inverse affine matrix to the click point and test against the
untransformed layer bounds.

Inverse of the 2×2 portion `[[a, c], [b, d]]`:

```
det = a*d - b*c
inv = [[d/det, -c/det], [-b/det, a/det]]
```

Then: `local = inv * (click - position)`. Test `local` against
`(0, 0, source_width, source_height)`.

## Scope Boundaries

**In scope:**
- Non-uniform scale (X/Y) and skew (X/Y) for image and text layers
- Canvas drag handles for interactive editing
- Shift modifier for free-form vs proportional corner scaling
- Backend compositor support via affine warp
- Frontend preview rendering via `ctx.transform()`
- Updated hit testing with inverse transform

**Out of scope:**
- Rotation (can be added as a future field + handle)
- Numeric property panel inputs (canvas handles only for now)
- Keyboard shortcuts for transforms
- Transform origin control (always top-left corner)
