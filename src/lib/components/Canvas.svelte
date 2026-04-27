<script lang="ts">
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { renderFrame } from '$lib/utils/canvas-renderer';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import * as cmd from '$lib/commands';
  import type { LayerInfo, Keyframe } from '$lib/types';

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

  // Initialise the 2D context once the canvas element is bound
  $effect(() => {
    if (canvas) {
      ctx = canvas.getContext('2d');
    }
  });

  // Re-render whenever the current frame index or layer list changes.
  //
  // A `stale` flag discards async callbacks from previous effect runs so
  // pausing playback doesn't keep rendering queued-up frames.
  $effect(() => {
    const frame = ui.currentFrame;
    const layers = project.layers;
    const previewExport = ui.previewExport;

    if (!ctx || !project.metadata) return;

    let stale = false;

    // Automatically use backend compositor when animated GIF layers are present,
    // since the frontend can't correctly synchronize GIF frames with the timeline
    // or apply transforms to browser-loaded animated GIFs.
    const hasAnimatedLayers = layers.some(l => l.is_animated);
    const useBackendCompositor = previewExport || hasAnimatedLayers;

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
      });
    } else {
      project.getFramePath(frame).then((framePath) => {
        if (stale || !ctx) return;
        renderFrame(ctx, framePath, layers, frame);
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

  function getTransformedCorners(layer: LayerInfo): [number, number][] {
    const interp = interpolateKeyframes(layer.keyframes, ui.currentFrame);
    const [tx, ty] = interp ? interp.position : layer.position;
    const { scale_x: sx, scale_y: sy, skew_x: kx, skew_y: ky } = layer;

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

    return [
      [tx, ty],
      [sx * w + tx, ky * w + ty],
      [kx * h + tx, sy * h + ty],
      [sx * w + kx * h + tx, ky * w + sy * h + ty],
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

        if (e.shiftKey) {
          // Shift: free-form resize (non-uniform)
          if (e.altKey) {
            // Alt+Shift: resize all layers non-uniformly
            const scaleXRatio = rawSx / handleOrigScaleX;
            const scaleYRatio = rawSy / handleOrigScaleY;
            project.layers = project.layers.map((l) => ({
              ...l,
              scale_x: l.scale_x * scaleXRatio,
              scale_y: l.scale_y * scaleYRatio,
            }));
          } else {
            // Shift only: resize selected layer non-uniformly
            project.layers = project.layers.map((l) =>
              l.id === handleLayerId
                ? { ...l, scale_x: rawSx, scale_y: rawSy }
                : l,
            );
          }
        } else {
          // Default: uniform resize (maintain aspect ratio)
          const origDiag = Math.sqrt(handleOrigScaleX ** 2 + handleOrigScaleY ** 2);
          const newDiag = Math.sqrt(rawSx ** 2 + rawSy ** 2);
          const ratio = origDiag > 0 ? newDiag / origDiag : 1;

          if (e.altKey) {
            // Alt: resize all layers uniformly
            project.layers = project.layers.map((l) => ({
              ...l,
              scale_x: l.scale_x * ratio,
              scale_y: l.scale_y * ratio,
            }));
          } else {
            // Default: resize selected layer uniformly
            project.layers = project.layers.map((l) =>
              l.id === handleLayerId
                ? { ...l, scale_x: handleOrigScaleX * ratio, scale_y: handleOrigScaleY * ratio }
                : l,
            );
          }
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
        // Alt was held: persist scale changes for all layers
        const updates = project.layers.map(l =>
          project.updateLayer(l.id, {
            scale_x: l.scale_x,
            scale_y: l.scale_y,
          })
        );
        await Promise.all(updates);
      } else {
        // Normal resize: persist only the selected layer
        const layer = project.layers.find((l) => l.id === handleLayerId);
        if (layer) {
          await project.updateLayer(handleLayerId, {
            scale_x: layer.scale_x,
            scale_y: layer.scale_y,
            skew_x: layer.skew_x,
            skew_y: layer.skew_y,
          });
        }
      }
      activeHandle = null;
      handleLayerId = null;
      wasResizingAllLayers = false;
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
