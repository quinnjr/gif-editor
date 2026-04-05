# Keyframe Animation for Overlay Layers

**Date:** 2026-04-04
**Status:** Approved

## Overview

Add keyframe-based animation for layer position and opacity. Users set
keyframes by dragging overlays on specific frames; intermediate frames
interpolate linearly. Diamond markers on the timeline show where
keyframes exist.

## Data Model

New struct shared by both layer types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub frame: usize,        // logical frame index
    pub position: (f64, f64),
    pub opacity: f64,
}
```

Both `ImageLayer` and `TextLayer` gain:

```rust
pub keyframes: Vec<Keyframe>,  // sorted by frame, default empty
```

**Behaviour rules:**

- `keyframes` empty → existing `position` and `opacity` apply uniformly
  (backwards-compatible).
- `keyframes` non-empty → interpolation:
  - Before first keyframe: first keyframe's values.
  - Between two keyframes: linear interpolation.
  - After last keyframe: last keyframe's values.

The existing `position` and `opacity` fields remain as the base values
used only when no keyframes exist.

## Interpolation

A single pure function used by both backend and frontend:

```
fn interpolate(keyframes, frame_index) -> (position, opacity)
```

1. If `keyframes` is empty, return `None` (caller uses base values).
2. If `frame_index <= keyframes[0].frame`, return `keyframes[0]` values.
3. If `frame_index >= keyframes[last].frame`, return `keyframes[last]` values.
4. Otherwise, find the two surrounding keyframes and lerp:
   ```
   t = (frame_index - kf_a.frame) / (kf_b.frame - kf_a.frame)
   x = kf_a.x + t * (kf_b.x - kf_a.x)
   y = kf_a.y + t * (kf_b.y - kf_a.y)
   opacity = kf_a.opacity + t * (kf_b.opacity - kf_a.opacity)
   ```

## Backend Compositor

Before compositing each layer, call `interpolate` to get effective
position and opacity for the current frame. If keyframes are present,
use the interpolated values instead of the layer's base `position` and
`opacity`. Scale and skew are NOT keyframed — they use the layer's
existing fields unchanged.

## Frontend Canvas Renderer

Same interpolation logic in `canvas-renderer.ts`. Before calling
`ctx.transform()`, compute the effective position and opacity from
keyframes. The transform call becomes:

```js
ctx.transform(layer.scale_x, layer.skew_y, layer.skew_x, layer.scale_y, tx, ty);
```

where `tx`, `ty` come from the interpolated position (or `layer.position`
if no keyframes).

## Canvas Interaction

Dragging the overlay now creates or updates a keyframe:

- On pointer up after a position drag: if the layer has any keyframes
  (or this is the first drag that creates them), upsert a keyframe at
  `ui.currentFrame` with the new position and current effective opacity.
- If the layer has no keyframes and the user simply drags, it updates
  `position` directly (existing behaviour).
- First keyframe creation: when the user drags for the first time and
  the layer has no keyframes, also create an initial keyframe at frame 0
  with the layer's original position/opacity, then create the keyframe
  at the current frame. This ensures interpolation has a starting point.

The opacity slider in `LayerItem.svelte` follows the same pattern: if
keyframes exist, create/update a keyframe at the current frame with the
new opacity and current interpolated position.

## Timeline Indicators

When a layer is selected, small diamond markers (`◆`) appear on the
thumbnail strip at each frame where a keyframe exists for that layer.

- Click a diamond: navigate to that frame.
- Hovering a diamond shows a small "×" to delete that keyframe.
- Deleting a keyframe removes it from the sorted list and the layer
  re-interpolates.
- If all keyframes are deleted, the layer reverts to its base
  `position` and `opacity`.

## Type Changes

### Backend

`LayerInfo` gains: `pub keyframes: Vec<Keyframe>`

`LayerUpdate` gains: `pub keyframes: Option<Vec<Keyframe>>`

`update_layer` applies keyframes when `Some`.

### Frontend

`LayerInfo` type gains: `keyframes: Keyframe[]`

`LayerUpdate` type gains: `keyframes?: Keyframe[]`

New TypeScript type:
```typescript
interface Keyframe {
    frame: number;
    position: [number, number];
    opacity: number;
}
```

## Frame Deletion Interaction

When frames are deleted or restored (`excluded_frames` changes), the
keyframe `frame` indices refer to logical frames. The `remap_layer_ranges`
logic must also remap keyframe frame indices using the same
`source_to_logical` mapping. Keyframes whose source frame is excluded
are removed.

## Scope Boundaries

**In scope:**
- Keyframe struct with frame, position, opacity
- Linear interpolation between keyframes
- Canvas drag creates/updates keyframes
- Opacity slider respects keyframes
- Diamond markers on timeline with delete
- Backend compositor + frontend renderer interpolation
- Keyframe remapping on frame deletion/restore

**Out of scope:**
- Easing curves (always linear)
- Keyframing scale/skew transforms
- Keyframe copy/paste
- Onion-skinning or ghost frames
- Keyframe editor panel with curves
