# Keyframe Animation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add keyframe-based position and opacity animation for overlay layers with linear interpolation, canvas drag-to-keyframe, and timeline diamond markers.

**Architecture:** Add a `Keyframe` struct and `keyframes: Vec<Keyframe>` to both layer types. A shared `interpolate_keyframes` function computes effective position/opacity for a given frame. The compositor and frontend renderer call it before applying transforms. Canvas drag creates keyframes instead of updating position directly when keyframes exist. Timeline shows diamond markers for selected layer's keyframes.

**Tech Stack:** Rust structs with serde, TypeScript types, Svelte 5 reactivity.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/layer.rs` | Modify | Add `Keyframe` struct, `keyframes` field on both layer types, `interpolate_keyframes` function |
| `src-tauri/src/compositor.rs` | Modify | Call `interpolate_keyframes` to get effective position/opacity per frame |
| `src-tauri/src/project.rs` | Modify | Add `keyframes` to `LayerInfo`, `LayerUpdate`, `From`, `update_layer`; remap keyframes on frame deletion |
| `src-tauri/tests/layer_test.rs` | Modify | Add keyframe interpolation tests |
| `src/lib/types.ts` | Modify | Add `Keyframe` type, add `keyframes` to `LayerInfo` and `LayerUpdate` |
| `src/lib/utils/canvas-renderer.ts` | Modify | Interpolate keyframes before rendering each layer |
| `src/lib/components/Canvas.svelte` | Modify | Drag creates keyframes; use interpolated position for handles |
| `src/lib/components/LayerItem.svelte` | Modify | Opacity slider creates keyframes when they exist |
| `src/lib/components/Timeline.svelte` | Modify | Diamond markers for keyframes on selected layer |

---

### Task 1: Keyframe Struct and Interpolation (Backend)

**Files:**
- Modify: `src-tauri/src/layer.rs`
- Test: `src-tauri/tests/layer_test.rs`

- [ ] **Step 1: Add Keyframe struct and interpolation function to layer.rs**

After the `Stroke` struct (line 8), add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub frame: usize,
    pub position: (f64, f64),
    pub opacity: f64,
}

/// Interpolate keyframes to get effective position and opacity at `frame_index`.
/// Returns `None` if `keyframes` is empty (caller should use base values).
pub fn interpolate_keyframes(keyframes: &[Keyframe], frame_index: usize) -> Option<((f64, f64), f64)> {
    if keyframes.is_empty() {
        return None;
    }

    // Before first keyframe
    if frame_index <= keyframes[0].frame {
        let kf = &keyframes[0];
        return Some((kf.position, kf.opacity));
    }

    // After last keyframe
    let last = &keyframes[keyframes.len() - 1];
    if frame_index >= last.frame {
        return Some((last.position, last.opacity));
    }

    // Between two keyframes — find the surrounding pair
    for i in 0..keyframes.len() - 1 {
        let a = &keyframes[i];
        let b = &keyframes[i + 1];
        if frame_index >= a.frame && frame_index <= b.frame {
            let span = (b.frame - a.frame) as f64;
            let t = if span > 0.0 {
                (frame_index - a.frame) as f64 / span
            } else {
                0.0
            };
            let x = a.position.0 + t * (b.position.0 - a.position.0);
            let y = a.position.1 + t * (b.position.1 - a.position.1);
            let opacity = a.opacity + t * (b.opacity - a.opacity);
            return Some(((x, y), opacity));
        }
    }

    Some((last.position, last.opacity))
}
```

- [ ] **Step 2: Add `keyframes` field to both layer structs**

In `ImageLayer`, add after `source_path`:
```rust
    pub keyframes: Vec<Keyframe>,
```

In `ImageLayer::new`, add before the closing `}`:
```rust
            keyframes: Vec::new(),
```

In `TextLayer`, add after `visible`:
```rust
    pub keyframes: Vec<Keyframe>,
```

In `TextLayer::new`, add before the closing `}`:
```rust
            keyframes: Vec::new(),
```

- [ ] **Step 3: Add helper method on Layer**

Add to `impl Layer`:
```rust
    pub fn keyframes(&self) -> &[Keyframe] {
        match self {
            Layer::Image(l) => &l.keyframes,
            Layer::Text(l) => &l.keyframes,
        }
    }
```

- [ ] **Step 4: Write interpolation tests**

Add to `src-tauri/tests/layer_test.rs`:

```rust
use gif_editor_lib::layer::{Keyframe, interpolate_keyframes};

#[test]
fn interpolate_empty_keyframes_returns_none() {
    assert_eq!(interpolate_keyframes(&[], 0), None);
}

#[test]
fn interpolate_single_keyframe_returns_its_values() {
    let kfs = vec![Keyframe { frame: 5, position: (10.0, 20.0), opacity: 0.5 }];
    // Before
    assert_eq!(interpolate_keyframes(&kfs, 0), Some(((10.0, 20.0), 0.5)));
    // At
    assert_eq!(interpolate_keyframes(&kfs, 5), Some(((10.0, 20.0), 0.5)));
    // After
    assert_eq!(interpolate_keyframes(&kfs, 10), Some(((10.0, 20.0), 0.5)));
}

#[test]
fn interpolate_two_keyframes_lerps() {
    let kfs = vec![
        Keyframe { frame: 0, position: (0.0, 0.0), opacity: 1.0 },
        Keyframe { frame: 10, position: (100.0, 50.0), opacity: 0.0 },
    ];
    let result = interpolate_keyframes(&kfs, 5).unwrap();
    assert!((result.0 .0 - 50.0).abs() < 0.01);
    assert!((result.0 .1 - 25.0).abs() < 0.01);
    assert!((result.1 - 0.5).abs() < 0.01);
}

#[test]
fn interpolate_clamps_before_and_after() {
    let kfs = vec![
        Keyframe { frame: 5, position: (10.0, 10.0), opacity: 0.8 },
        Keyframe { frame: 15, position: (20.0, 20.0), opacity: 0.2 },
    ];
    // Before first
    let before = interpolate_keyframes(&kfs, 0).unwrap();
    assert_eq!(before, ((10.0, 10.0), 0.8));
    // After last
    let after = interpolate_keyframes(&kfs, 20).unwrap();
    assert_eq!(after, ((20.0, 20.0), 0.2));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test layer_test -v`

Expected: all tests pass (3 original + 4 new).

- [ ] **Step 6: Commit**

```
feat(layer): add Keyframe struct with linear interpolation

Keyframe holds frame index, position, and opacity. The
interpolate_keyframes function returns the effective values for a
given frame index using linear interpolation between surrounding
keyframes, clamping before/after the first/last. Both ImageLayer
and TextLayer gain a keyframes: Vec<Keyframe> field (default empty).
```

---

### Task 2: Compositor Keyframe Support

**Files:**
- Modify: `src-tauri/src/compositor.rs`

- [ ] **Step 1: Use interpolated position/opacity in compositor**

At the top of `compositor.rs`, add the import:

```rust
use crate::layer::{Layer, interpolate_keyframes};
```

(Replace the existing `use crate::layer::Layer;`.)

In the `Layer::Image` arm, after getting `src` and the scale/skew values, replace the lines that read `img_layer.position` and `img_layer.opacity` by computing effective values:

```rust
                let (pos, opacity) = match interpolate_keyframes(&img_layer.keyframes, frame_index) {
                    Some((p, o)) => (p, o),
                    None => (img_layer.position, img_layer.opacity),
                };
```

Then use `pos` and `opacity` instead of `img_layer.position` and `img_layer.opacity` in the `composite_rgba_buffer` and `affine_composite` calls.

In the `Layer::Text` arm, do the same:

```rust
                let (pos, opacity) = match interpolate_keyframes(&text_layer.keyframes, frame_index) {
                    Some((p, o)) => (p, o),
                    None => (text_layer.position, text_layer.opacity),
                };
```

And use `pos` and `opacity` instead of `text_layer.position` and `text_layer.opacity`.

- [ ] **Step 2: Run compositor tests**

Run: `cargo test --test compositor_test -v`

Expected: all 9 tests pass (no keyframes = base values = same behaviour).

- [ ] **Step 3: Commit**

```
feat(compositor): use interpolated keyframe values for position and opacity

When a layer has keyframes, the compositor computes effective position
and opacity via interpolate_keyframes for the current frame index.
Layers without keyframes continue to use their base values.
```

---

### Task 3: Update LayerInfo, LayerUpdate, and update_layer

**Files:**
- Modify: `src-tauri/src/project.rs`

- [ ] **Step 1: Add keyframes to LayerInfo**

In the `LayerInfo` struct, add after `source_path`:
```rust
    pub keyframes: Vec<Keyframe>,
```

Add the import at the top of project.rs (after the existing layer imports):
```rust
use crate::layer::{ImageLayer, Layer, Keyframe, Stroke, TextLayer};
```
(Add `Keyframe` to the existing import.)

In `From<&Layer> for LayerInfo`, add to both arms (Image and Text), after `source_path`:

For Image arm:
```rust
                keyframes: l.keyframes.clone(),
```

For Text arm:
```rust
                keyframes: l.keyframes.clone(),
```

- [ ] **Step 2: Add keyframes to LayerUpdate**

In `LayerUpdate`, add after `stroke`:
```rust
    pub keyframes: Option<Vec<Keyframe>>,
```

- [ ] **Step 3: Apply keyframes in update_layer**

In `update_layer`, add to both arms (Image and Text), after the stroke handling:

```rust
                if let Some(v) = changes.keyframes {
                    l.keyframes = v;
                }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -v 2>&1 | tail -20`

Expected: all pass.

- [ ] **Step 5: Commit**

```
feat(project): propagate keyframes through LayerInfo, LayerUpdate, update_layer
```

---

### Task 4: Remap Keyframes on Frame Deletion

**Files:**
- Modify: `src-tauri/src/project.rs`

- [ ] **Step 1: Add keyframe remapping to remap_layer_ranges**

The existing `remap_layer_ranges` method remaps `frame_range`. Extend it to also remap keyframe frame indices.

After the line `for (layer, (ns, ne)) in self.layers.iter_mut().zip(new_ranges) {`, inside each match arm, after setting `l.frame_range`, add keyframe remapping:

```rust
                l.keyframes = l.keyframes.iter().filter_map(|kf| {
                    self.source_to_logical(
                        // The keyframe frame index was logical before exclusion change.
                        // We already converted it to source in the snapshot step.
                        kf.frame
                    ).map(|new_frame| Keyframe {
                        frame: new_frame,
                        position: kf.position,
                        opacity: kf.opacity,
                    })
                }).collect();
```

Wait — this won't work because `remap_layer_ranges` receives source ranges but keyframe indices are logical. We need to convert keyframes to source indices before modifying exclusions, just like we do for frame ranges.

Actually, the approach is simpler: the `delete_frames` and `restore_frames` methods already snapshot layer source ranges before modifying exclusions. We need to also snapshot keyframe frame indices as source indices, then remap them after.

Modify `delete_frames`: after the existing `layer_source_ranges` snapshot, add a keyframe snapshot:

```rust
        let layer_source_keyframes: Vec<Vec<Keyframe>> = self
            .layers
            .iter()
            .map(|l| {
                l.keyframes()
                    .iter()
                    .filter_map(|kf| {
                        self.logical_to_source(kf.frame).map(|src| Keyframe {
                            frame: src,
                            position: kf.position,
                            opacity: kf.opacity,
                        })
                    })
                    .collect()
            })
            .collect();
```

Then after `self.remap_layer_ranges(...)`, add:

```rust
        self.remap_layer_keyframes(&layer_source_keyframes);
```

Do the same in `restore_frames` — add the keyframe snapshot before modifying exclusions and the remap call after.

Add a new private method:

```rust
    fn remap_layer_keyframes(&mut self, source_keyframes: &[Vec<Keyframe>]) {
        for (layer, src_kfs) in self.layers.iter_mut().zip(source_keyframes.iter()) {
            let new_kfs: Vec<Keyframe> = src_kfs
                .iter()
                .filter_map(|kf| {
                    self.source_to_logical(kf.frame).map(|new_frame| Keyframe {
                        frame: new_frame,
                        position: kf.position,
                        opacity: kf.opacity,
                    })
                })
                .collect();
            match layer {
                Layer::Image(l) => l.keyframes = new_kfs,
                Layer::Text(l) => l.keyframes = new_kfs,
            }
        }
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -v 2>&1 | tail -20`

Expected: all pass.

- [ ] **Step 3: Commit**

```
feat(project): remap keyframe indices on frame deletion and restore

Keyframe frame indices are converted to source indices before
modifying the exclusion set, then remapped back to logical indices
afterward. Keyframes on excluded frames are dropped.
```

---

### Task 5: Frontend Types and Canvas Renderer

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/utils/canvas-renderer.ts`

- [ ] **Step 1: Add Keyframe type and update interfaces**

In `src/lib/types.ts`, add after the `Stroke` interface:

```typescript
export interface Keyframe {
  frame: number;
  position: [number, number];
  opacity: number;
}
```

Add to `LayerInfo` (after `source_path`):
```typescript
  keyframes: Keyframe[];
```

Add to `LayerUpdate` (after `stroke`):
```typescript
  keyframes?: Keyframe[];
```

- [ ] **Step 2: Add interpolation helper and update canvas renderer**

In `src/lib/utils/canvas-renderer.ts`, add after the imports:

```typescript
import type { LayerInfo, Keyframe } from '$lib/types';
```

(Replace the existing `import type { LayerInfo } from '$lib/types';`.)

Add the interpolation helper after `loadImage`:

```typescript
function interpolateKeyframes(
  keyframes: Keyframe[],
  frameIndex: number,
): { position: [number, number]; opacity: number } | null {
  if (!keyframes || keyframes.length === 0) return null;

  if (frameIndex <= keyframes[0].frame) {
    return { position: keyframes[0].position, opacity: keyframes[0].opacity };
  }

  const last = keyframes[keyframes.length - 1];
  if (frameIndex >= last.frame) {
    return { position: last.position, opacity: last.opacity };
  }

  for (let i = 0; i < keyframes.length - 1; i++) {
    const a = keyframes[i];
    const b = keyframes[i + 1];
    if (frameIndex >= a.frame && frameIndex <= b.frame) {
      const span = b.frame - a.frame;
      const t = span > 0 ? (frameIndex - a.frame) / span : 0;
      return {
        position: [
          a.position[0] + t * (b.position[0] - a.position[0]),
          a.position[1] + t * (b.position[1] - a.position[1]),
        ],
        opacity: a.opacity + t * (b.opacity - a.opacity),
      };
    }
  }

  return { position: last.position, opacity: last.opacity };
}
```

- [ ] **Step 3: Use interpolation in renderFrame**

In the layer rendering loop, replace the position/opacity extraction:

```typescript
    const interp = interpolateKeyframes(layer.keyframes, frameIndex);
    const [tx, ty] = interp ? interp.position : layer.position;
    const layerOpacity = interp ? interp.opacity : layer.opacity;

    ctx.save();
    ctx.globalAlpha = layerOpacity;
    ctx.transform(layer.scale_x, layer.skew_y, layer.skew_x, layer.scale_y, tx, ty);
```

(Replace the existing `const [tx, ty] = layer.position;` and `ctx.globalAlpha = layer.opacity;` lines.)

- [ ] **Step 4: Commit**

```
feat(frontend): add keyframe interpolation to types and canvas renderer

New Keyframe type added. The canvas renderer interpolates position
and opacity from keyframes when present, falling back to base values
when empty.
```

---

### Task 6: Canvas Drag Creates Keyframes

**Files:**
- Modify: `src/lib/components/Canvas.svelte`

- [ ] **Step 1: Import interpolateKeyframes and Keyframe**

At the top of Canvas.svelte, add to imports:

```typescript
  import type { LayerInfo, Keyframe } from '$lib/types';
```

(Replace the existing `import type { LayerInfo } from '$lib/types';`.)

Add after the existing imports:

```typescript
  function interpolateKeyframes(
    keyframes: Keyframe[],
    frameIndex: number,
  ): { position: [number, number]; opacity: number } | null {
    if (!keyframes || keyframes.length === 0) return null;
    if (frameIndex <= keyframes[0].frame) {
      return { position: keyframes[0].position, opacity: keyframes[0].opacity };
    }
    const last = keyframes[keyframes.length - 1];
    if (frameIndex >= last.frame) {
      return { position: last.position, opacity: last.opacity };
    }
    for (let i = 0; i < keyframes.length - 1; i++) {
      const a = keyframes[i];
      const b = keyframes[i + 1];
      if (frameIndex >= a.frame && frameIndex <= b.frame) {
        const span = b.frame - a.frame;
        const t = span > 0 ? (frameIndex - a.frame) / span : 0;
        return {
          position: [
            a.position[0] + t * (b.position[0] - a.position[0]),
            a.position[1] + t * (b.position[1] - a.position[1]),
          ],
          opacity: a.opacity + t * (b.opacity - a.opacity),
        };
      }
    }
    return { position: last.position, opacity: last.opacity };
  }
```

- [ ] **Step 2: Update getTransformedCorners and findLayerAtPoint**

In `getTransformedCorners`, replace the position extraction at the top:

```typescript
    const interp = interpolateKeyframes(layer.keyframes, ui.currentFrame);
    const [tx, ty] = interp ? interp.position : layer.position;
```

In `findLayerAtPoint`, replace the position extraction:

```typescript
      const interp = interpolateKeyframes(layer.keyframes, frame);
      const [tx, ty] = interp ? interp.position : layer.position;
```

- [ ] **Step 3: Update onPointerUp to create keyframes on drag**

In `onPointerUp`, replace the position drag section (the `if (!isDragging || dragLayerId === null)` block):

```typescript
    if (!isDragging || dragLayerId === null) return;

    const layer = project.layers.find((l) => l.id === dragLayerId);
    if (layer) {
      const newPos = layer.position;
      const frame = ui.currentFrame;

      if (layer.keyframes.length > 0) {
        // Update or insert keyframe at current frame
        const interp = interpolateKeyframes(layer.keyframes, frame);
        const currentOpacity = interp ? interp.opacity : layer.opacity;
        const newKfs = upsertKeyframe(layer.keyframes, {
          frame,
          position: newPos,
          opacity: currentOpacity,
        });
        await project.updateLayer(dragLayerId, { keyframes: newKfs });
      } else if (frame > 0) {
        // First keyframe creation: seed frame 0 with original position,
        // then add the new position at current frame
        const kfs: Keyframe[] = [
          { frame: 0, position: [dragOriginX, dragOriginY], opacity: layer.opacity },
          { frame, position: newPos, opacity: layer.opacity },
        ];
        await project.updateLayer(dragLayerId, { keyframes: kfs });
      } else {
        // Frame 0, no keyframes — just update position directly
        await project.updateLayer(dragLayerId, { position: newPos });
      }
    }

    isDragging = false;
    dragLayerId = null;
```

Add the `upsertKeyframe` helper after the `interpolateKeyframes` function:

```typescript
  function upsertKeyframe(keyframes: Keyframe[], kf: Keyframe): Keyframe[] {
    const filtered = keyframes.filter((k) => k.frame !== kf.frame);
    const result = [...filtered, kf].sort((a, b) => a.frame - b.frame);
    return result;
  }
```

- [ ] **Step 4: Commit**

```
feat(canvas): drag creates keyframes for position animation

When a layer has keyframes, dragging creates/updates a keyframe at
the current frame. First drag on a non-zero frame seeds an initial
keyframe at frame 0. Handle positions and hit testing use
interpolated values.
```

---

### Task 7: Opacity Slider Keyframe Support

**Files:**
- Modify: `src/lib/components/LayerItem.svelte`

- [ ] **Step 1: Update opacity handler to create keyframes**

In `src/lib/components/LayerItem.svelte`, add imports:

```typescript
  import { ui } from '$lib/stores/ui.svelte';
  import type { Keyframe } from '$lib/types';
```

Replace the `handleOpacity` function:

```typescript
  async function handleOpacity(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    if (layer.keyframes.length > 0) {
      // Update/insert keyframe at current frame with new opacity
      const frame = ui.currentFrame;
      const interp = interpolateKeyframes(layer.keyframes, frame);
      const pos = interp ? interp.position : layer.position;
      const newKfs = upsertKeyframe(layer.keyframes, {
        frame,
        position: pos,
        opacity: value,
      });
      await project.updateLayer(layer.id, { keyframes: newKfs });
    } else {
      await project.updateLayer(layer.id, { opacity: value });
    }
  }
```

Add the helper functions (same as Canvas.svelte):

```typescript
  function interpolateKeyframes(
    keyframes: Keyframe[],
    frameIndex: number,
  ): { position: [number, number]; opacity: number } | null {
    if (!keyframes || keyframes.length === 0) return null;
    if (frameIndex <= keyframes[0].frame) {
      return { position: keyframes[0].position, opacity: keyframes[0].opacity };
    }
    const last = keyframes[keyframes.length - 1];
    if (frameIndex >= last.frame) {
      return { position: last.position, opacity: last.opacity };
    }
    for (let i = 0; i < keyframes.length - 1; i++) {
      const a = keyframes[i];
      const b = keyframes[i + 1];
      if (frameIndex >= a.frame && frameIndex <= b.frame) {
        const span = b.frame - a.frame;
        const t = span > 0 ? (frameIndex - a.frame) / span : 0;
        return {
          position: [
            a.position[0] + t * (b.position[0] - a.position[0]),
            a.position[1] + t * (b.position[1] - a.position[1]),
          ],
          opacity: a.opacity + t * (b.opacity - a.opacity),
        };
      }
    }
    return { position: last.position, opacity: last.opacity };
  }

  function upsertKeyframe(keyframes: Keyframe[], kf: Keyframe): Keyframe[] {
    const filtered = keyframes.filter((k) => k.frame !== kf.frame);
    return [...filtered, kf].sort((a, b) => a.frame - b.frame);
  }
```

- [ ] **Step 2: Show interpolated opacity in slider**

Update the opacity slider to show the effective value:

```svelte
      {@const interp = layer.keyframes.length > 0
        ? interpolateKeyframes(layer.keyframes, ui.currentFrame)
        : null}
      <label class="flex items-center gap-2 text-xs text-zinc-300">
        Opacity
        <input type="range" min="0" max="1" step="0.05" value={interp ? interp.opacity : layer.opacity}
          oninput={handleOpacity} class="flex-1 accent-blue-500" />
        <span class="w-8 text-right">{Math.round((interp ? interp.opacity : layer.opacity) * 100)}%</span>
      </label>
```

- [ ] **Step 3: Commit**

```
feat(layer-item): opacity slider creates keyframes when animation is active
```

---

### Task 8: Timeline Keyframe Indicators

**Files:**
- Modify: `src/lib/components/Timeline.svelte`

- [ ] **Step 1: Add diamond markers to thumbnail strip**

In `Timeline.svelte`, inside the `{#each thumbnails as src, i}` block, after the selection overlay and before the frame number `<span>`, add a keyframe indicator:

```svelte
            {#if selectedLayer && selectedLayer.keyframes.some((kf) => kf.frame === i)}
              <div class="absolute left-1/2 top-0 -translate-x-1/2 text-[8px] leading-none text-yellow-400"
                title="Keyframe">&#9670;</div>
            {/if}
```

- [ ] **Step 2: Add keyframe delete on right-click**

Add a context menu handler to the thumbnail div. Replace the `onclick` on the thumbnail div:

```svelte
            onclick={(e) => toggleFrameSelection(i, e)}
            oncontextmenu={(e) => handleThumbnailContextMenu(i, e)}
```

Add the handler function in the `<script>` section:

```typescript
  function handleThumbnailContextMenu(index: number, e: MouseEvent) {
    e.preventDefault();
    if (!selectedLayer) return;
    const kfIndex = selectedLayer.keyframes.findIndex((kf) => kf.frame === index);
    if (kfIndex === -1) return;
    // Remove the keyframe
    const newKfs = selectedLayer.keyframes.filter((kf) => kf.frame !== index);
    project.updateLayer(selectedLayer.id, { keyframes: newKfs });
  }
```

- [ ] **Step 3: Commit**

```
feat(timeline): show diamond markers for keyframes, right-click to delete

Yellow diamond indicators appear on thumbnails where the selected
layer has keyframes. Right-clicking a thumbnail with a keyframe
removes it.
```

---

### Task 9: Verify and Final Commit

- [ ] **Step 1: Run full test suite**

Run: `cargo test 2>&1 | grep "test result"`

Expected: all pass.

- [ ] **Step 2: Run svelte-check**

Run: `npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep -c ERROR`

Expected: 3 (pre-existing).

- [ ] **Step 3: Manual smoke test**

Run: `pnpm tauri dev`

Test:
1. Open a GIF, add an image layer
2. On frame 0, position the overlay at top-left
3. Navigate to the last frame, drag the overlay to bottom-right
4. Play back — verify the overlay smoothly moves from top-left to bottom-right
5. Check that diamond markers appear on frames 0 and the last frame
6. Right-click a diamond marker — verify keyframe is deleted and position snaps
7. On frame 0, adjust opacity to 0.5 — verify it creates a keyframe
8. Navigate to middle frame — verify interpolated opacity
9. Toggle "Preview Export" — verify backend compositor matches frontend
10. Delete some frames — verify keyframes remap correctly

- [ ] **Step 4: Final commit**

```
feat: add keyframe animation for layer position and opacity

Layers can now be animated by setting keyframes at specific frames.
Dragging an overlay on any frame creates a keyframe; frames between
keyframes interpolate linearly. Opacity slider creates keyframes
when animation is active.

Yellow diamond markers on the timeline show where keyframes exist.
Right-click a diamond to delete that keyframe. Both the backend
compositor and frontend renderer use the same interpolation logic.
Keyframe indices are remapped when frames are deleted or restored.
```
