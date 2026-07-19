<script lang="ts">
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { renderFrame, interpolateKeyframes, onFontsReady } from '$lib/utils/canvas-renderer';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import * as cmd from '$lib/commands';
  import type { LayerInfo, Keyframe } from '$lib/types';

  function upsertKeyframe(keyframes: Keyframe[], kf: Keyframe): Keyframe[] {
    const filtered = keyframes.filter((k) => k.frame !== kf.frame);
    return [...filtered, kf].sort((a, b) => a.frame - b.frame);
  }

  let canvas = $state<HTMLCanvasElement | undefined>(undefined);
  let ctx = $state<CanvasRenderingContext2D | null>(null);

  // Drag state
  let isDragging = $state(false);
  let dragLayerId = $state<string | null>(null);
  let dragStartX = $state(0);
  let dragStartY = $state(0);
  let dragOriginX = $state(0);
  let dragOriginY = $state(0);

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
  let wasResizingAllLayers = $state(false);

  // Snapshot of every layer's scale at corner-gesture start. The Alt
  // "resize all layers" mode assigns absolute values (snapshot × ratio) on
  // each pointermove so repeated moves don't compound exponentially.
  let resizeSnapshot: Map<string, { scale_x: number; scale_y: number }> | null = null;
  // Net scale ratio of the current gesture relative to its start, persisted
  // through the backend's scale_all_layers (which multiplies) on pointerup.
  let resizeAllRatioX = 1;
  let resizeAllRatioY = 1;

  // Initialise the 2D context once the canvas element is bound
  $effect(() => {
    if (canvas) {
      ctx = canvas.getContext('2d');
    }
  });

  // Bumped when a bundled FontFace finishes loading (canvas-renderer has
  // already dropped the stale fallback-font text rasters by then); the
  // render effect below reads it so text repaints with the real typeface.
  let fontsVersion = $state(0);
  // onFontsReady returns an unsubscribe function, used as effect cleanup.
  $effect(() => onFontsReady(() => { fontsVersion += 1; }));

  // Re-render whenever the current frame index or layer list changes.
  //
  // A `stale` flag discards async callbacks from previous effect runs so
  // pausing playback doesn't keep rendering queued-up frames.
  $effect(() => {
    const frame = ui.currentFrame;
    const layers = project.layers;
    const previewExport = ui.previewExport;
    // Depend on fontsVersion so a completed FontFace load repaints the frame.
    void fontsVersion;

    if (!ctx || !project.metadata) return;

    let stale = false;

    // Automatically use backend compositor when animated GIF layers are present,
    // since the frontend can't correctly synchronize GIF frames with the timeline
    // or apply transforms to browser-loaded animated GIFs.
    const hasAnimatedLayers = layers.some(l => l.is_animated);

    // While a drag/resize gesture is in progress every pointermove reassigns
    // project.layers, so routing each re-run through the backend compositor
    // would issue an unthrottled IPC composite per pointer event. During the
    // gesture, render client-side instead — the client path draws only the
    // FIRST frame of animated overlay layers, which is the accepted
    // gesture-time tradeoff — then let the effect re-run for one final
    // backend composite on gesture end (isDragging/activeHandle are $state,
    // so clearing them in onPointerUp re-triggers this effect).
    const gestureActive = isDragging || activeHandle !== null;
    const useBackendCompositor = (previewExport || hasAnimatedLayers) && !gestureActive;

    if (useBackendCompositor) {
      cmd.renderComposite(frame).then((dataUrl) => {
        if (stale || !ctx) return;
        const img = new Image();
        img.onload = () => {
          if (stale || !ctx) return;
          const { width, height } = ctx.canvas;
          ctx.clearRect(0, 0, width, height);
          ctx.drawImage(img, 0, 0, width, height);
        };
        img.src = convertFileSrc(dataUrl);
      }).catch((err) => {
        // "export in progress" is a designed condition: the backend refuses
        // uncached frame loads while exporting. Keep showing the last
        // rendered frame instead of raising an error toast.
        if (!stale && !String(err).includes('export in progress')) {
          ui.showError(`Failed to render frame: ${err}`);
        }
      });
    } else {
      project.getFramePath(frame).then((framePath) => {
        if (stale || !ctx) return;
        renderFrame(ctx, framePath, layers, frame);
      }).catch((err) => {
        // Same designed condition as above — no toast during an export.
        if (!stale && !String(err).includes('export in progress')) {
          ui.showError(`Failed to load frame: ${err}`);
        }
      });
    }

    return () => { stale = true; };
  });

  // --- Hit testing helpers ---

  const HANDLE_SIZE = 8;
  const HANDLE_GRAB = 12;

  function getCanvasCoords(e: PointerEvent): [number, number] {
    if (!canvas) return [0, 0];
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    return [
      (e.clientX - rect.left) * scaleX,
      (e.clientY - rect.top) * scaleY,
    ];
  }

  // Combined rotation × scale/skew matrix, identical to the one applied by
  // canvas-renderer.ts and the backend compositor:
  //   dst_x = a*src_x + c*src_y + tx
  //   dst_y = b*src_x + d*src_y + ty
  function getLayerMatrix(layer: LayerInfo): { a: number; b: number; c: number; d: number } {
    const theta = (layer.rotation ?? 0) * (Math.PI / 180);
    const cosT = Math.cos(theta);
    const sinT = Math.sin(theta);
    const { scale_x: sx, scale_y: sy, skew_x: kx, skew_y: ky } = layer;
    return {
      a: cosT * sx - sinT * ky,
      b: sinT * sx + cosT * ky,
      c: cosT * kx - sinT * sy,
      d: sinT * kx + cosT * sy,
    };
  }

  function getTransformedCorners(layer: LayerInfo): [number, number][] {
    const interp = interpolateKeyframes(layer.keyframes, ui.currentFrame);
    const [tx, ty] = interp ? interp.position : layer.position;

    if (layer.layer_type === 'flare') {
      const hs = 60;
      return [
        [tx - hs, ty - hs],
        [tx + hs, ty - hs],
        [tx - hs, ty + hs],
        [tx + hs, ty + hs],
      ];
    }

    let w: number, h: number;
    if (layer.layer_type === 'image') {
      w = layer.source_width ?? 100;
      h = layer.source_height ?? 100;
    } else {
      const fontSize = layer.font_size ?? 48;
      w = (layer.text?.length ?? 1) * fontSize * 0.6;
      h = fontSize;
    }

    const { a, b, c, d } = getLayerMatrix(layer);
    return [
      [tx, ty],
      [a * w + tx, b * w + ty],
      [c * h + tx, d * h + ty],
      [a * w + c * h + tx, b * w + d * h + ty],
    ];
  }

  function getHandlePositions(layer: LayerInfo): { type: HandleType; x: number; y: number }[] {
    if (layer.layer_type === 'flare') return [];
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

  function findLayerAtPoint(x: number, y: number): LayerInfo | null {
    const frame = ui.currentFrame;
    for (let i = project.layers.length - 1; i >= 0; i--) {
      const layer = project.layers[i];
      if (!layer.visible) continue;
      const [start, end] = layer.frame_range;
      if (frame < start || frame > end) continue;

      const interp = interpolateKeyframes(layer.keyframes, frame);
      const [tx, ty] = interp ? interp.position : layer.position;

      if (layer.layer_type === 'flare') {
        const dx = x - tx;
        const dy = y - ty;
        if (dx * dx + dy * dy <= 60 * 60) return layer;
        continue;
      }

      // Inverse of the full rotation × scale/skew matrix [[a, c], [b, d]]
      const { a, b, c, d } = getLayerMatrix(layer);
      const det = a * d - b * c;
      if (Math.abs(det) < 1e-9) continue;

      const localX = (d * (x - tx) - c * (y - ty)) / det;
      const localY = (-b * (x - tx) + a * (y - ty)) / det;

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

  // --- Pointer event handlers ---

  function onPointerDown(e: PointerEvent) {
    const [x, y] = getCanvasCoords(e);

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
      resizeSnapshot = new Map(
        project.layers.map((l) => [l.id, { scale_x: l.scale_x, scale_y: l.scale_y }]),
      );
      resizeAllRatioX = 1;
      resizeAllRatioY = 1;
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
      return;
    }

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
        const rawSx = handleOrigScaleX + dx / w;
        const rawSy = handleOrigScaleY + dy / h;

        // Track if Alt was held for onPointerUp
        wasResizingAllLayers = e.altKey;

        // New absolute scale for the selected layer, and the net ratio of
        // this gesture relative to its start (used for the Alt all-layers
        // mode). Both are derived from the constant gesture-start values so
        // every pointermove is idempotent.
        let selScaleX: number;
        let selScaleY: number;
        let ratioX: number;
        let ratioY: number;

        if (e.shiftKey) {
          // Shift: free-form resize (non-uniform)
          selScaleX = rawSx;
          selScaleY = rawSy;
          ratioX = handleOrigScaleX !== 0 ? rawSx / handleOrigScaleX : 1;
          ratioY = handleOrigScaleY !== 0 ? rawSy / handleOrigScaleY : 1;
        } else {
          // Default: uniform resize (maintain aspect ratio)
          const origDiag = Math.sqrt(handleOrigScaleX ** 2 + handleOrigScaleY ** 2);
          const newDiag = Math.sqrt(rawSx ** 2 + rawSy ** 2);
          const ratio = origDiag > 0 ? newDiag / origDiag : 1;
          selScaleX = handleOrigScaleX * ratio;
          selScaleY = handleOrigScaleY * ratio;
          ratioX = ratio;
          ratioY = ratio;
        }

        const snap = resizeSnapshot;
        if (e.altKey && snap) {
          // Alt: resize all layers. Assign absolute values from the
          // gesture-start snapshot so repeated moves don't compound.
          resizeAllRatioX = ratioX;
          resizeAllRatioY = ratioY;
          project.layers = project.layers.map((l) => {
            const s = snap.get(l.id);
            return s ? { ...l, scale_x: s.scale_x * ratioX, scale_y: s.scale_y * ratioY } : l;
          });
        } else {
          // Resize only the selected layer, restoring any other layers an
          // earlier Alt move within this gesture may have scaled.
          project.layers = project.layers.map((l) => {
            if (l.id === handleLayerId) {
              return { ...l, scale_x: selScaleX, scale_y: selScaleY };
            }
            const s = snap?.get(l.id);
            return s && (l.scale_x !== s.scale_x || l.scale_y !== s.scale_y)
              ? { ...l, scale_x: s.scale_x, scale_y: s.scale_y }
              : l;
          });
        }
      } else {
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
    if (activeHandle && handleLayerId) {
      if (wasResizingAllLayers) {
        // Alt was held: persist the whole gesture as ONE history entry via
        // the backend's scale_all_layers, which multiplies current scales by
        // the given factors. Revert local layers to the gesture-start
        // snapshot first, then replace them with the backend's result.
        const snap = resizeSnapshot;
        if (snap) {
          project.layers = project.layers.map((l) => {
            const s = snap.get(l.id);
            return s ? { ...l, scale_x: s.scale_x, scale_y: s.scale_y } : l;
          });
        }
        if (resizeAllRatioX !== 1 || resizeAllRatioY !== 1) {
          try {
            await project.scaleAllLayers(resizeAllRatioX, resizeAllRatioY);
          } catch (err) {
            ui.showError(`Failed to resize layers: ${err}`);
          }
        }
      } else {
        // Normal resize: persist only the selected layer
        const layer = project.layers.find((l) => l.id === handleLayerId);
        if (layer) {
          try {
            await project.updateLayer(handleLayerId, {
              scale_x: layer.scale_x,
              scale_y: layer.scale_y,
              skew_x: layer.skew_x,
              skew_y: layer.skew_y,
            });
          } catch (err) {
            ui.showError(`Failed to resize layer: ${err}`);
          }
        }
      }
      activeHandle = null;
      handleLayerId = null;
      wasResizingAllLayers = false;
      resizeSnapshot = null;
      return;
    }

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
        // First keyframe creation: seed frame 0, then current frame
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
  }
</script>

{#if project.metadata}
  <!-- Pointer events on the wrapper so they fire for both the canvas and
       the SVG handle overlay (handles have pointer-events-auto). -->
  <div class="relative max-h-full max-w-full cursor-crosshair"
    role="application"
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}>
    <canvas
      bind:this={canvas}
      width={project.metadata.width}
      height={project.metadata.height}
      class="max-h-full max-w-full"
    ></canvas>

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
          <polygon
            points="{[corners[0], corners[1], corners[3], corners[2]].map(([cx, cy]) => `${cx * ratioX},${cy * ratioY}`).join(' ')}"
            fill="none" stroke="#60a5fa" stroke-width="1" stroke-dasharray="4 2" />
          {#each handles as h (h.type)}
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
