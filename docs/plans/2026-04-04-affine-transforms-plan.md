# Affine Transforms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add non-uniform scaling and skew to image/text layers with interactive canvas drag handles.

**Architecture:** Replace the single `scale: f64` field on layers with `scale_x`, `scale_y`, `skew_x`, `skew_y`. Update the backend compositor to apply affine warps (with a fast path for identity transforms). Update the frontend canvas renderer to use `ctx.transform()`. Add 8 drag handles to Canvas.svelte for interactive editing.

**Tech Stack:** Rust `image` crate (affine warp via manual pixel sampling — no new crate dependency), Canvas 2D `transform()` API, Svelte 5 reactivity.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/layer.rs` | Modify | Replace `scale` with `scale_x`, `scale_y`, `skew_x`, `skew_y` on both layer structs |
| `src-tauri/src/compositor.rs` | Modify | Affine warp compositing with fast identity path |
| `src-tauri/src/text_renderer.rs` | Modify | Use `font_size` directly (remove `* layer.scale` bake-in) |
| `src-tauri/src/project.rs` | Modify | Update `LayerInfo`, `LayerUpdate`, `From<&Layer>`, `update_layer` |
| `src-tauri/tests/layer_test.rs` | Modify | Update default-value assertions |
| `src-tauri/tests/compositor_test.rs` | Modify | Update existing tests + add affine transform tests |
| `src/lib/types.ts` | Modify | Replace `scale` with four transform fields in `LayerInfo` and `LayerUpdate` |
| `src/lib/utils/canvas-renderer.ts` | Modify | Use `ctx.transform()` for layer rendering |
| `src/lib/components/Canvas.svelte` | Modify | Add drag handles, inverse hit testing, handle interactions |

---

### Task 1: Update the Data Model (Backend)

**Files:**
- Modify: `src-tauri/src/layer.rs`
- Test: `src-tauri/tests/layer_test.rs`

- [ ] **Step 1: Update `ImageLayer` and `TextLayer` structs**

In `src-tauri/src/layer.rs`, replace `scale: f64` with the four new fields on both structs.

`ImageLayer` fields (replace line 17 `pub scale: f64,`):
```rust
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
```

`ImageLayer::new` (replace line 34 `scale: 1.0,`):
```rust
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
```

`TextLayer` fields (replace line 54 `pub scale: f64,`):
```rust
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
```

`TextLayer::new` (replace line 74 `scale: 1.0,`):
```rust
            scale_x: 1.0,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
```

- [ ] **Step 2: Update the layer tests**

In `src-tauri/tests/layer_test.rs`, replace the `scale` assertions.

`image_layer_default_values` test — replace `assert_eq!(layer.scale, 1.0);` (line 9) with:
```rust
    assert_eq!(layer.scale_x, 1.0);
    assert_eq!(layer.scale_y, 1.0);
    assert_eq!(layer.skew_x, 0.0);
    assert_eq!(layer.skew_y, 0.0);
```

`text_layer_default_values` test — replace `assert_eq!(layer.scale, 1.0);` (line 21) with:
```rust
    assert_eq!(layer.scale_x, 1.0);
    assert_eq!(layer.scale_y, 1.0);
    assert_eq!(layer.skew_x, 0.0);
    assert_eq!(layer.skew_y, 0.0);
```

- [ ] **Step 3: Run tests to verify layer model compiles and passes**

Run: `cargo test --test layer_test -v`

This will fail with compile errors in compositor, text_renderer, and project.rs that still reference `scale`. That's expected — we fix those in subsequent tasks. To verify just the layer tests, temporarily comment out the other modules or run:

Run: `cargo test --test layer_test 2>&1; echo "Exit: $?"`

Expected: layer_test passes (3 tests). Other tests may fail due to `scale` references — that's expected and fixed in Tasks 2–4.

- [ ] **Step 4: Commit**

```
feat(layer): replace uniform scale with affine transform fields

The single `scale: f64` field on ImageLayer and TextLayer is replaced
with `scale_x`, `scale_y`, `skew_x`, `skew_y` to support non-uniform
scaling and shear transforms. All default to identity (scale 1.0,
skew 0.0).
```

---

### Task 2: Update `LayerInfo`, `LayerUpdate`, and `update_layer`

**Files:**
- Modify: `src-tauri/src/project.rs`

- [ ] **Step 1: Update `LayerInfo` struct**

In `src-tauri/src/project.rs`, in the `LayerInfo` struct definition (around line 38), replace:
```rust
    pub scale: f64,
```
with:
```rust
    pub scale_x: f64,
    pub scale_y: f64,
    pub skew_x: f64,
    pub skew_y: f64,
```

- [ ] **Step 2: Update `From<&Layer> for LayerInfo`**

In the `From` impl (around line 54), update both arms.

For `Layer::Image(l)` — replace `scale: l.scale,` with:
```rust
                scale_x: l.scale_x,
                scale_y: l.scale_y,
                skew_x: l.skew_x,
                skew_y: l.skew_y,
```

For `Layer::Text(l)` — replace `scale: l.scale,` with:
```rust
                scale_x: l.scale_x,
                scale_y: l.scale_y,
                skew_x: l.skew_x,
                skew_y: l.skew_y,
```

- [ ] **Step 3: Update `LayerUpdate` struct**

In the `LayerUpdate` struct (around line 100), replace:
```rust
    pub scale: Option<f64>,
```
with:
```rust
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub skew_x: Option<f64>,
    pub skew_y: Option<f64>,
```

- [ ] **Step 4: Update `update_layer` method**

In `Project::update_layer` (around line 248), update both the `Layer::Image(l)` and `Layer::Text(l)` arms. In each arm, replace:
```rust
                if let Some(v) = changes.scale {
                    l.scale = v;
                }
```
with:
```rust
                if let Some(v) = changes.scale_x {
                    l.scale_x = v;
                }
                if let Some(v) = changes.scale_y {
                    l.scale_y = v;
                }
                if let Some(v) = changes.skew_x {
                    l.skew_x = v;
                }
                if let Some(v) = changes.skew_y {
                    l.skew_y = v;
                }
```

Apply this replacement in **both** match arms (Image and Text).

- [ ] **Step 5: Commit**

```
feat(project): propagate affine transform fields through LayerInfo and LayerUpdate

LayerInfo, LayerUpdate, the From impl, and update_layer all now use
scale_x/scale_y/skew_x/skew_y instead of the old uniform scale field.
```

---

### Task 3: Update the Text Renderer

**Files:**
- Modify: `src-tauri/src/text_renderer.rs`

- [ ] **Step 1: Remove scale bake-in from text renderer**

In `src-tauri/src/text_renderer.rs`, line 27, replace:
```rust
    let scale = PxScale::from((layer.font_size * layer.scale) as f32);
```
with:
```rust
    let scale = PxScale::from(layer.font_size as f32);
```

The text renderer now rasterises at the base `font_size`. The affine transform (including `scale_x`/`scale_y`) is applied by the compositor in Task 4, which warps the rasterised text image.

- [ ] **Step 2: Run text renderer tests**

Run: `cargo test --test text_renderer_test -v`

Expected: all pass — the test constructs TextLayers with default scale values, and `font_size * 1.0 == font_size`, so behaviour is identical.

- [ ] **Step 3: Commit**

```
refactor(text-renderer): remove scale bake-in from font rasterisation

Text is now rasterised at base font_size. Scaling is handled by the
compositor's affine warp, which applies scale_x/scale_y uniformly
across both image and text layers. This avoids double-scaling and
enables non-uniform text scaling.
```

---

### Task 4: Affine Warp Compositor

**Files:**
- Modify: `src-tauri/src/compositor.rs`
- Test: `src-tauri/tests/compositor_test.rs`

- [ ] **Step 1: Write failing tests for affine compositing**

Add these tests to `src-tauri/tests/compositor_test.rs`:

```rust
#[test]
fn composite_image_with_scale_x() {
    let base = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
    let overlay = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255]));
    let mut layer = ImageLayer::new("blue".into(), 4, 4);
    layer.image_data = Some(overlay);
    layer.position = (0.0, 0.0);
    layer.scale_x = 2.0;
    layer.scale_y = 1.0;
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    // Pixel at (7, 2) should be blue — inside the 8×4 scaled region
    assert_eq!(*result.get_pixel(7, 2), Rgba([0, 0, 255, 255]));
    // Pixel at (9, 2) should be red — outside the 8×4 scaled region
    assert_eq!(*result.get_pixel(9, 2), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_identity_transform_matches_original() {
    // Identity transform (scale 1,1 skew 0,0) should produce the same result
    // as the fast path.
    let base = red_10x10();
    let overlay = blue_5x5();
    let mut layer = ImageLayer::new("blue".into(), 5, 5);
    layer.image_data = Some(overlay);
    layer.position = (2.0, 3.0);
    layer.frame_range = (0, 0);
    // scale_x=1, scale_y=1, skew_x=0, skew_y=0 are defaults
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    assert_eq!(*result.get_pixel(2, 3), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
    assert_eq!(*result.get_pixel(6, 7), Rgba([0, 0, 255, 255]));
    assert_eq!(*result.get_pixel(7, 8), Rgba([255, 0, 0, 255]));
}

#[test]
fn composite_image_with_skew_x() {
    let base = RgbaImage::from_pixel(30, 30, Rgba([255, 0, 0, 255]));
    let overlay = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 255, 255]));
    let mut layer = ImageLayer::new("blue".into(), 10, 10);
    layer.image_data = Some(overlay);
    layer.position = (5.0, 5.0);
    layer.skew_x = 0.5; // moderate horizontal shear
    layer.frame_range = (0, 0);
    let layers = vec![Layer::Image(layer)];
    let result = composite_frame(&base, &layers, 0);
    // The bottom-right area of the original 10x10 should shift right due to skew.
    // Pixel at (5, 5) — top-left of layer — should still be blue
    assert_eq!(*result.get_pixel(5, 5), Rgba([0, 0, 255, 255]));
    // Pixel at (0, 0) should still be red (untouched)
    assert_eq!(*result.get_pixel(0, 0), Rgba([255, 0, 0, 255]));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test compositor_test -v 2>&1 | tail -20`

Expected: the three new tests fail (compile errors or assertion failures). Existing tests may also fail because `layer.scale` no longer exists — that's expected.

- [ ] **Step 3: Update existing compositor tests**

The existing tests that construct `ImageLayer` use default values (scale=1.0), which are now `scale_x=1.0, scale_y=1.0`. Since the defaults match, no field assignments need changing — the existing tests should compile as-is after the compositor is updated.

- [ ] **Step 4: Implement affine warp compositing**

Replace the contents of `src-tauri/src/compositor.rs` with:

```rust
use image::{Rgba, RgbaImage};

use crate::layer::Layer;

/// Composite all visible, in-range layers onto a clone of `base` for the
/// given `frame_index`.  Returns a new image; the base is not mutated.
pub fn composite_frame(base: &RgbaImage, layers: &[Layer], frame_index: usize) -> RgbaImage {
    let mut target = base.clone();

    for layer in layers {
        if !layer.visible() {
            continue;
        }

        let (start, end) = layer.frame_range();
        if frame_index < start || frame_index > end {
            continue;
        }

        match layer {
            Layer::Image(img_layer) => {
                let Some(ref src) = img_layer.image_data else {
                    continue;
                };

                let sx = img_layer.scale_x;
                let sy = img_layer.scale_y;
                let kx = img_layer.skew_x;
                let ky = img_layer.skew_y;

                if is_identity(sx, sy, kx, ky) {
                    // Fast path: no transform needed, just blit at position.
                    composite_rgba_buffer(
                        &mut target,
                        src,
                        img_layer.position,
                        img_layer.opacity,
                    );
                } else {
                    affine_composite(
                        &mut target,
                        src,
                        img_layer.position,
                        sx, sy, kx, ky,
                        img_layer.opacity,
                    );
                }
            }
            Layer::Text(text_layer) => {
                if let Ok(text_img) = crate::text_renderer::render_text(text_layer) {
                    let sx = text_layer.scale_x;
                    let sy = text_layer.scale_y;
                    let kx = text_layer.skew_x;
                    let ky = text_layer.skew_y;

                    if is_identity(sx, sy, kx, ky) {
                        composite_rgba_buffer(
                            &mut target,
                            &text_img,
                            text_layer.position,
                            text_layer.opacity,
                        );
                    } else {
                        affine_composite(
                            &mut target,
                            &text_img,
                            text_layer.position,
                            sx, sy, kx, ky,
                            text_layer.opacity,
                        );
                    }
                }
            }
        }
    }

    target
}

/// Returns `true` when the 2×2 transform portion is the identity matrix.
fn is_identity(sx: f64, sy: f64, kx: f64, ky: f64) -> bool {
    (sx - 1.0).abs() < f64::EPSILON
        && (sy - 1.0).abs() < f64::EPSILON
        && kx.abs() < f64::EPSILON
        && ky.abs() < f64::EPSILON
}

/// Composite `src` onto `target` using the affine transform defined by
/// `(scale_x, scale_y, skew_x, skew_y)` with translation `position`.
///
/// The matrix maps source → destination:
///
/// ```text
///   dst_x = scale_x * src_x + skew_x * src_y + tx
///   dst_y = skew_y  * src_x + scale_y * src_y + ty
/// ```
///
/// We iterate over the output bounding box and use the inverse matrix to
/// sample source pixels with bilinear interpolation.
fn affine_composite(
    target: &mut RgbaImage,
    src: &RgbaImage,
    position: (f64, f64),
    sx: f64,
    sy: f64,
    kx: f64,
    ky: f64,
    opacity: f64,
) {
    let (tw, th) = (target.width() as i64, target.height() as i64);
    let (sw, sh) = (src.width() as f64, src.height() as f64);
    let (tx, ty) = position;

    // Compute the bounding box of the four transformed source corners.
    let corners = [
        (0.0, 0.0),
        (sw, 0.0),
        (0.0, sh),
        (sw, sh),
    ];

    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for (cx, cy) in &corners {
        let dx = sx * cx + kx * cy + tx;
        let dy = ky * cx + sy * cy + ty;
        min_x = min_x.min(dx);
        min_y = min_y.min(dy);
        max_x = max_x.max(dx);
        max_y = max_y.max(dy);
    }

    // Clamp to target bounds.
    let x0 = (min_x.floor() as i64).max(0);
    let y0 = (min_y.floor() as i64).max(0);
    let x1 = (max_x.ceil() as i64).min(tw);
    let y1 = (max_y.ceil() as i64).min(th);

    // Inverse of the 2×2 portion [[sx, kx], [ky, sy]].
    let det = sx * sy - kx * ky;
    if det.abs() < 1e-12 {
        return; // Degenerate transform, nothing to draw.
    }
    let inv_a = sy / det;
    let inv_b = -ky / det;
    let inv_c = -kx / det;
    let inv_d = sx / det;

    for dst_y in y0..y1 {
        for dst_x in x0..x1 {
            // Map destination pixel back to source coordinates.
            let rx = dst_x as f64 - tx;
            let ry = dst_y as f64 - ty;
            let src_xf = inv_a * rx + inv_c * ry;
            let src_yf = inv_b * rx + inv_d * ry;

            // Bounds check (with 0.5px margin for bilinear).
            if src_xf < -0.5 || src_yf < -0.5 || src_xf >= sw - 0.5 || src_yf >= sh - 0.5 {
                continue;
            }

            let src_pixel = bilinear_sample(src, src_xf, src_yf);
            if src_pixel[3] == 0 {
                continue;
            }

            let dst_pixel = target.get_pixel_mut(dst_x as u32, dst_y as u32);
            let effective_alpha = (src_pixel[3] as f64 / 255.0) * opacity;
            *dst_pixel = alpha_blend(dst_pixel, &src_pixel, effective_alpha);
        }
    }
}

/// Bilinear sampling from an RGBA image at fractional coordinates.
/// Coordinates outside the image are clamped to the nearest edge pixel.
fn bilinear_sample(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x = x.max(0.0).min(w - 1.0);
    let y = y.max(0.0).min(h - 1.0);

    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(img.width() - 1);
    let y1 = (y0 + 1).min(img.height() - 1);

    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let p00 = img.get_pixel(x0, y0);
    let p10 = img.get_pixel(x1, y0);
    let p01 = img.get_pixel(x0, y1);
    let p11 = img.get_pixel(x1, y1);

    let lerp = |a: u8, b: u8, c: u8, d: u8| -> u8 {
        let top = a as f64 * (1.0 - fx) + b as f64 * fx;
        let bot = c as f64 * (1.0 - fx) + d as f64 * fx;
        let val = top * (1.0 - fy) + bot * fy;
        val.round() as u8
    };

    Rgba([
        lerp(p00[0], p10[0], p01[0], p11[0]),
        lerp(p00[1], p10[1], p01[1], p11[1]),
        lerp(p00[2], p10[2], p01[2], p11[2]),
        lerp(p00[3], p10[3], p01[3], p11[3]),
    ])
}

/// Blend `buffer` onto `target` at `position` with the given `opacity`.
/// Fast path for identity transforms (no scale, no skew).
pub fn composite_rgba_buffer(
    target: &mut RgbaImage,
    buffer: &RgbaImage,
    position: (f64, f64),
    opacity: f64,
) {
    let (tw, th) = target.dimensions();
    let off_x = position.0.round() as i64;
    let off_y = position.1.round() as i64;

    for (bx, by, src_pixel) in buffer.enumerate_pixels() {
        let tx = off_x + bx as i64;
        let ty = off_y + by as i64;

        if tx < 0 || ty < 0 || tx >= tw as i64 || ty >= th as i64 {
            continue;
        }

        let dst_pixel = target.get_pixel_mut(tx as u32, ty as u32);
        let effective_alpha = (src_pixel[3] as f64 / 255.0) * opacity;
        *dst_pixel = alpha_blend(dst_pixel, src_pixel, effective_alpha);
    }
}

/// Standard "alpha over" composite.
fn alpha_blend(dst: &Rgba<u8>, src: &Rgba<u8>, src_alpha: f64) -> Rgba<u8> {
    let inv = 1.0 - src_alpha;
    let r = (src[0] as f64 * src_alpha + dst[0] as f64 * inv).round() as u8;
    let g = (src[1] as f64 * src_alpha + dst[1] as f64 * inv).round() as u8;
    let b = (src[2] as f64 * src_alpha + dst[2] as f64 * inv).round() as u8;
    Rgba([r, g, b, dst[3]])
}
```

- [ ] **Step 5: Run all compositor tests**

Run: `cargo test --test compositor_test -v`

Expected: all 8 tests pass (5 existing + 3 new).

- [ ] **Step 6: Commit**

```
feat(compositor): implement affine warp compositing

The compositor now applies the full affine transform (scale_x,
scale_y, skew_x, skew_y + position) using inverse mapping with
bilinear interpolation. An identity fast path preserves the
original blit performance for untransformed layers.
```

---

### Task 5: Update the Frontend Types

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Update `LayerInfo` interface**

In `src/lib/types.ts`, replace `scale: number;` (line 19) with:
```typescript
  scale_x: number;
  scale_y: number;
  skew_x: number;
  skew_y: number;
```

- [ ] **Step 2: Update `LayerUpdate` interface**

In the same file, replace `scale?: number;` (line 35) with:
```typescript
  scale_x?: number;
  scale_y?: number;
  skew_x?: number;
  skew_y?: number;
```

- [ ] **Step 3: Commit**

```
feat(types): replace scale with affine transform fields in frontend types
```

---

### Task 6: Update the Frontend Canvas Renderer

**Files:**
- Modify: `src/lib/utils/canvas-renderer.ts`

- [ ] **Step 1: Replace layer rendering with affine transforms**

Replace the layer rendering loop body (lines 36–59) in `src/lib/utils/canvas-renderer.ts`. The full updated `renderFrame` function:

```typescript
export async function renderFrame(
  ctx: CanvasRenderingContext2D,
  framePath: string,
  layers: LayerInfo[],
  frameIndex: number,
) {
  const { width, height } = ctx.canvas;
  ctx.clearRect(0, 0, width, height);

  // Draw base frame
  const frameSrc = convertFileSrc(framePath);
  const baseImg = await loadImage(frameSrc);
  ctx.drawImage(baseImg, 0, 0, width, height);

  // Draw layers in order (back to front)
  for (const layer of layers) {
    if (!layer.visible) continue;
    const [start, end] = layer.frame_range;
    if (frameIndex < start || frameIndex > end) continue;

    const [tx, ty] = layer.position;

    ctx.save();
    ctx.globalAlpha = layer.opacity;
    // Apply affine: ctx.transform(a, b, c, d, e, f)
    // a=scale_x, b=skew_y, c=skew_x, d=scale_y, e=tx, f=ty
    ctx.transform(layer.scale_x, layer.skew_y, layer.skew_x, layer.scale_y, tx, ty);

    if (layer.layer_type === 'image') {
      if (!layer.source_path) { ctx.restore(); continue; }
      const img = await loadImage(convertFileSrc(layer.source_path));
      ctx.drawImage(img, 0, 0);
    } else if (layer.layer_type === 'text') {
      const fontSize = layer.font_size ?? 48;
      ctx.font = `${fontSize}px "${layer.font_family ?? 'Impact'}", sans-serif`;
      ctx.textBaseline = 'top';
      if (layer.stroke) {
        ctx.strokeStyle = `rgba(${layer.stroke.color.join(',')})`;
        ctx.lineWidth = layer.stroke.width * 2;
        ctx.lineJoin = 'round';
        ctx.strokeText(layer.text ?? '', 0, 0);
      }
      const [r, g, b, a] = layer.color ?? [255, 255, 255, 255];
      ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
      ctx.fillText(layer.text ?? '', 0, 0);
    }

    ctx.restore();
  }
}
```

Key changes from the original:
- `ctx.save()`/`ctx.restore()` wraps each layer (isolates transform state)
- `ctx.transform(scale_x, skew_y, skew_x, scale_y, tx, ty)` replaces manual position/size
- Images draw at `(0, 0)` — the transform handles placement and scaling
- Text uses base `font_size` without multiplying by scale — the transform handles it

- [ ] **Step 2: Commit**

```
feat(canvas-renderer): use affine transforms for layer rendering

Layers are now rendered with ctx.transform() using scale_x, scale_y,
skew_x, skew_y, and position. Images and text draw at the origin;
the transform matrix handles placement, scaling, and shear.
```

---

### Task 7: Update Canvas Hit Testing and Drag-to-Move

**Files:**
- Modify: `src/lib/components/Canvas.svelte`

- [ ] **Step 1: Update `findLayerAtPoint` to use inverse affine transform**

Replace the `findLayerAtPoint` function (lines 75–99) in `Canvas.svelte`:

```typescript
  function findLayerAtPoint(x: number, y: number): LayerInfo | null {
    const frame = ui.currentFrame;
    for (let i = project.layers.length - 1; i >= 0; i--) {
      const layer = project.layers[i];
      if (!layer.visible) continue;
      const [start, end] = layer.frame_range;
      if (frame < start || frame > end) continue;

      const [tx, ty] = layer.position;
      const { scale_x: sx, scale_y: sy, skew_x: kx, skew_y: ky } = layer;

      // Inverse of [[sx, kx], [ky, sy]]
      const det = sx * sy - kx * ky;
      if (Math.abs(det) < 1e-9) continue;

      const localX = (sy * (x - tx) - kx * (y - ty)) / det;
      const localY = (-ky * (x - tx) + sx * (y - ty)) / det;

      let w: number, h: number;
      if (layer.layer_type === 'image') {
        w = layer.source_width ?? 0;
        h = layer.source_height ?? 0;
      } else {
        const fontSize = layer.font_size ?? 48;
        w = (layer.text?.length ?? 1) * fontSize * 0.6;
        h = fontSize;
      }

      if (localX >= 0 && localX <= w && localY >= 0 && localY <= h) {
        return layer;
      }
    }
    return null;
  }
```

- [ ] **Step 2: Run `cargo test` and `npx svelte-check` to verify no regressions**

Run: `cargo test 2>&1 | tail -5`
Run: `npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep ERROR`

Expected: all Rust tests pass. Svelte check shows only the 3 pre-existing errors (vite.config.ts, test files).

- [ ] **Step 3: Commit**

```
feat(canvas): update hit testing to use inverse affine transform

Click coordinates are mapped to local layer space using the inverse
of the layer's 2×2 affine matrix before bounds testing. This makes
hit testing work correctly with scaled and skewed layers.
```

---

### Task 8: Canvas Drag Handles for Scale and Skew

**Files:**
- Modify: `src/lib/components/Canvas.svelte`

- [ ] **Step 1: Add handle state and types**

At the top of the `<script>` block in `Canvas.svelte`, after the existing drag state variables (after line 18), add:

```typescript
  // Handle interaction state
  type HandleType =
    | 'tl' | 'tr' | 'bl' | 'br'       // corner handles (scale)
    | 'top' | 'bottom' | 'left' | 'right'; // edge handles (skew)
  let activeHandle = $state<HandleType | null>(null);
  let handleLayerId = $state<string | null>(null);
  let handleStartX = $state(0);
  let handleStartY = $state(0);
  let handleOrigScaleX = $state(1);
  let handleOrigScaleY = $state(1);
  let handleOrigSkewX = $state(0);
  let handleOrigSkewY = $state(0);
```

- [ ] **Step 2: Add handle geometry helpers**

After the `getCanvasCoords` function, add:

```typescript
  const HANDLE_SIZE = 8;
  const HANDLE_GRAB = 12; // generous grab radius

  /** Compute the 4 corners of a layer's transformed bounding box. */
  function getTransformedCorners(layer: LayerInfo): [number, number][] {
    const [tx, ty] = layer.position;
    const { scale_x: sx, scale_y: sy, skew_x: kx, skew_y: ky } = layer;

    let w: number, h: number;
    if (layer.layer_type === 'image') {
      w = layer.source_width ?? 0;
      h = layer.source_height ?? 0;
    } else {
      const fontSize = layer.font_size ?? 48;
      w = (layer.text?.length ?? 1) * fontSize * 0.6;
      h = fontSize;
    }

    // Transform each corner: dst = M * src + t
    return [
      [tx, ty],                                         // top-left
      [sx * w + tx, ky * w + ty],                       // top-right
      [kx * h + tx, sy * h + ty],                       // bottom-left
      [sx * w + kx * h + tx, ky * w + sy * h + ty],     // bottom-right
    ];
  }

  /** Get the 8 handle positions: 4 corners + 4 edge midpoints. */
  function getHandlePositions(layer: LayerInfo): { type: HandleType; x: number; y: number }[] {
    const [tl, tr, bl, br] = getTransformedCorners(layer);
    const mid = (a: [number, number], b: [number, number]): [number, number] =>
      [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];

    return [
      { type: 'tl', x: tl[0], y: tl[1] },
      { type: 'tr', x: tr[0], y: tr[1] },
      { type: 'bl', x: bl[0], y: bl[1] },
      { type: 'br', x: br[0], y: br[1] },
      { type: 'top', x: mid(tl, tr)[0], y: mid(tl, tr)[1] },
      { type: 'bottom', x: mid(bl, br)[0], y: mid(bl, br)[1] },
      { type: 'left', x: mid(tl, bl)[0], y: mid(tl, bl)[1] },
      { type: 'right', x: mid(tr, br)[0], y: mid(tr, br)[1] },
    ];
  }

  /** Hit-test canvas coords against the 8 handles. */
  function findHandleAtPoint(x: number, y: number): { handle: HandleType; layerId: string } | null {
    const sel = ui.selectedLayerId ? project.layers.find((l) => l.id === ui.selectedLayerId) : null;
    if (!sel) return null;

    for (const h of getHandlePositions(sel)) {
      if (Math.abs(x - h.x) <= HANDLE_GRAB && Math.abs(y - h.y) <= HANDLE_GRAB) {
        return { handle: h.type, layerId: sel.id };
      }
    }
    return null;
  }
```

- [ ] **Step 3: Rewrite pointer event handlers**

Replace the three pointer handlers (`onPointerDown`, `onPointerMove`, `onPointerUp`) with:

```typescript
  function onPointerDown(e: PointerEvent) {
    const [x, y] = getCanvasCoords(e);

    // 1. Check handles first (only when a layer is selected)
    const handleHit = findHandleAtPoint(x, y);
    if (handleHit) {
      const layer = project.layers.find((l) => l.id === handleHit.layerId);
      if (!layer) return;
      activeHandle = handleHit.handle;
      handleLayerId = handleHit.layerId;
      handleStartX = x;
      handleStartY = y;
      handleOrigScaleX = layer.scale_x;
      handleOrigScaleY = layer.scale_y;
      handleOrigSkewX = layer.skew_x;
      handleOrigSkewY = layer.skew_y;
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
      return;
    }

    // 2. Check layer body
    const hit = findLayerAtPoint(x, y);
    if (hit) {
      ui.selectLayer(hit.id);
      isDragging = true;
      dragLayerId = hit.id;
      dragStartX = x;
      dragStartY = y;
      dragOriginX = hit.position[0];
      dragOriginY = hit.position[1];
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    } else {
      ui.selectLayer(null);
    }
  }

  function onPointerMove(e: PointerEvent) {
    // Handle drag
    if (activeHandle && handleLayerId) {
      const [x, y] = getCanvasCoords(e);
      const layer = project.layers.find((l) => l.id === handleLayerId);
      if (!layer) return;

      const dx = x - handleStartX;
      const dy = y - handleStartY;

      let w: number, h: number;
      if (layer.layer_type === 'image') {
        w = layer.source_width ?? 1;
        h = layer.source_height ?? 1;
      } else {
        const fontSize = layer.font_size ?? 48;
        w = (layer.text?.length ?? 1) * fontSize * 0.6;
        h = fontSize;
      }

      const isCorner = ['tl', 'tr', 'bl', 'br'].includes(activeHandle);
      if (isCorner) {
        // Corner: scale. Shift = free-form, default = proportional.
        const rawSx = handleOrigScaleX + dx / w;
        const rawSy = handleOrigScaleY + dy / h;

        if (e.shiftKey) {
          // Free-form: independent X/Y
          project.layers = project.layers.map((l) =>
            l.id === handleLayerId
              ? { ...l, scale_x: rawSx, scale_y: rawSy }
              : l,
          );
        } else {
          // Proportional: use diagonal ratio
          const origDiag = Math.sqrt(handleOrigScaleX ** 2 + handleOrigScaleY ** 2);
          const newDiag = Math.sqrt(rawSx ** 2 + rawSy ** 2);
          const ratio = origDiag > 0 ? newDiag / origDiag : 1;
          project.layers = project.layers.map((l) =>
            l.id === handleLayerId
              ? { ...l, scale_x: handleOrigScaleX * ratio, scale_y: handleOrigScaleY * ratio }
              : l,
          );
        }
      } else {
        // Edge: skew.
        if (activeHandle === 'top' || activeHandle === 'bottom') {
          const newSkewX = handleOrigSkewX + dx / h;
          project.layers = project.layers.map((l) =>
            l.id === handleLayerId ? { ...l, skew_x: newSkewX } : l,
          );
        } else {
          const newSkewY = handleOrigSkewY + dy / w;
          project.layers = project.layers.map((l) =>
            l.id === handleLayerId ? { ...l, skew_y: newSkewY } : l,
          );
        }
      }
      return;
    }

    // Position drag
    if (!isDragging || dragLayerId === null) return;

    const [x, y] = getCanvasCoords(e);
    const dx = x - dragStartX;
    const dy = y - dragStartY;
    const newX = dragOriginX + dx;
    const newY = dragOriginY + dy;

    project.layers = project.layers.map((l) =>
      l.id === dragLayerId
        ? { ...l, position: [newX, newY] as [number, number] }
        : l,
    );
  }

  async function onPointerUp(_e: PointerEvent) {
    // Finish handle drag
    if (activeHandle && handleLayerId) {
      const layer = project.layers.find((l) => l.id === handleLayerId);
      if (layer) {
        await project.updateLayer(handleLayerId, {
          scale_x: layer.scale_x,
          scale_y: layer.scale_y,
          skew_x: layer.skew_x,
          skew_y: layer.skew_y,
        });
      }
      activeHandle = null;
      handleLayerId = null;
      return;
    }

    // Finish position drag
    if (!isDragging || dragLayerId === null) return;

    const layer = project.layers.find((l) => l.id === dragLayerId);
    if (layer) {
      await project.updateLayer(dragLayerId, { position: layer.position });
    }

    isDragging = false;
    dragLayerId = null;
  }
```

- [ ] **Step 4: Add handle overlay rendering**

Replace the template (lines 152–164) with:

```svelte
{#if project.metadata}
  <div class="relative max-h-full max-w-full">
    <canvas
      bind:this={canvas}
      width={project.metadata.width}
      height={project.metadata.height}
      class="max-h-full max-w-full cursor-crosshair"
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
    ></canvas>

    <!-- Handle overlay: drawn via a second effect into a transparent canvas -->
    {#if ui.selectedLayerId}
      {@const sel = project.layers.find((l) => l.id === ui.selectedLayerId)}
      {#if sel && canvas}
        {@const corners = getTransformedCorners(sel)}
        {@const handles = getHandlePositions(sel)}
        {@const rect = canvas.getBoundingClientRect()}
        {@const ratioX = rect.width / (project.metadata?.width ?? 1)}
        {@const ratioY = rect.height / (project.metadata?.height ?? 1)}
        <svg class="pointer-events-none absolute left-0 top-0"
          width={rect.width} height={rect.height}
          viewBox="0 0 {rect.width} {rect.height}">
          <!-- Bounding parallelogram (tl → tr → br → bl for correct winding) -->
          <polygon
            points="{[corners[0], corners[1], corners[3], corners[2]].map(([cx, cy]) => `${cx * ratioX},${cy * ratioY}`).join(' ')}"
            fill="none" stroke="#60a5fa" stroke-width="1" stroke-dasharray="4 2" />
          <!-- Handles -->
          {#each handles as h}
            <rect
              x={h.x * ratioX - HANDLE_SIZE / 2}
              y={h.y * ratioY - HANDLE_SIZE / 2}
              width={HANDLE_SIZE} height={HANDLE_SIZE}
              fill="#60a5fa" stroke="#1e3a5f" stroke-width="1"
              class="pointer-events-auto cursor-pointer" />
          {/each}
        </svg>
      {/if}
    {/if}
  </div>
{:else}
  <p class="text-zinc-500">Open a GIF to get started</p>
{/if}
```

- [ ] **Step 5: Run `npx svelte-check` to verify**

Run: `npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep -c ERROR`

Expected: 3 (the same pre-existing errors).

- [ ] **Step 6: Commit**

```
feat(canvas): add interactive drag handles for scale and skew

Eight handles appear around the selected layer's transformed bounding
box. Corner handles scale the layer (proportional by default, Shift
for free-form X/Y). Edge handles adjust skew. Hit testing,
drag-to-move, and pointer up all account for the affine transform.
```

---

### Task 9: Fix Remaining `scale` References

**Files:**
- Modify: `src/lib/components/LayerItem.svelte` (if it references `scale`)
- Modify: `src/lib/components/Canvas.svelte` (clean up any leftover refs)

- [ ] **Step 1: Search for remaining `scale` references**

Run: `grep -rn '\.scale[^_]' src/lib/ src-tauri/src/ --include='*.ts' --include='*.svelte' --include='*.rs' | grep -v node_modules | grep -v target`

Fix any remaining references to the old `scale` field. Based on the codebase review, the following may still reference it:

In `src/lib/components/LayerItem.svelte` — the position display doesn't reference `scale`, so no change needed.

In `src/lib/components/ExportDialog.svelte` — no `scale` references.

- [ ] **Step 2: Run full test suite**

Run: `cargo test 2>&1 | tail -20`

Expected: all tests pass.

Run: `npx svelte-check --tsconfig ./tsconfig.json 2>&1 | grep -c ERROR`

Expected: 3 (pre-existing).

- [ ] **Step 3: Commit (if any fixes)**

```
chore: clean up remaining references to old uniform scale field
```

---

### Task 10: Integration Test

- [ ] **Step 1: Manual smoke test**

Run: `pnpm tauri dev`

Test the following:
1. Open a GIF or MP4
2. Add an image layer — verify it renders at default scale
3. Select the layer — verify 8 handles appear
4. Drag a corner handle — verify proportional scaling
5. Hold Shift + drag corner — verify free-form X/Y scaling
6. Drag a top/bottom edge handle — verify horizontal skew
7. Drag a left/right edge handle — verify vertical skew
8. Click the canvas outside the layer — verify handles disappear
9. Toggle "Preview Export" — verify the backend compositor matches the frontend preview
10. Export as GIF — verify the output includes the transforms

- [ ] **Step 2: Final commit**

```
feat: add non-uniform scale and skew transforms to overlay layers

Layers now support independent X/Y scaling and horizontal/vertical
shear via decomposed affine transform fields. Interactive canvas
drag handles provide corner scaling (proportional by default, Shift
for free-form) and edge skewing. Both the backend compositor
(bilinear warp) and frontend renderer (Canvas 2D transform API)
apply the full affine matrix.

Closes the overlay transform feature request.
```
