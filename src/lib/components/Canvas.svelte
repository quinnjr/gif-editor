<script lang="ts">
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { renderFrame } from '$lib/utils/canvas-renderer';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import * as cmd from '$lib/commands';
  import type { LayerInfo } from '$lib/types';

  let canvas = $state<HTMLCanvasElement | undefined>(undefined);
  let ctx = $state<CanvasRenderingContext2D | null>(null);

  // Drag state
  let isDragging = $state(false);
  let dragLayerId = $state<string | null>(null);
  let dragStartX = $state(0);
  let dragStartY = $state(0);
  let dragOriginX = $state(0);
  let dragOriginY = $state(0);

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

    if (previewExport) {
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

  function findLayerAtPoint(x: number, y: number): LayerInfo | null {
    // Test in reverse order so top-most layer wins
    const frame = ui.currentFrame;
    for (let i = project.layers.length - 1; i >= 0; i--) {
      const layer = project.layers[i];
      if (!layer.visible) continue;
      const [start, end] = layer.frame_range;
      if (frame < start || frame > end) continue;

      const [lx, ly] = layer.position;

      if (layer.layer_type === 'image') {
        const w = (layer.source_width ?? 0) * layer.scale;
        const h = (layer.source_height ?? 0) * layer.scale;
        if (x >= lx && x <= lx + w && y >= ly && y <= ly + h) return layer;
      } else if (layer.layer_type === 'text') {
        const fontSize = (layer.font_size ?? 48) * layer.scale;
        // Rough estimate: character width ~0.6 * fontSize
        const estW = (layer.text?.length ?? 1) * fontSize * 0.6;
        const estH = fontSize;
        if (x >= lx && x <= lx + estW && y >= ly && y <= ly + estH) return layer;
      }
    }
    return null;
  }

  // --- Pointer event handlers ---

  function onPointerDown(e: PointerEvent) {
    const [x, y] = getCanvasCoords(e);
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
    if (!isDragging || dragLayerId === null) return;

    const [x, y] = getCanvasCoords(e);
    const dx = x - dragStartX;
    const dy = y - dragStartY;
    const newX = dragOriginX + dx;
    const newY = dragOriginY + dy;

    // Update the layer position locally in the store for instant feedback
    project.layers = project.layers.map((l) =>
      l.id === dragLayerId
        ? { ...l, position: [newX, newY] as [number, number] }
        : l,
    );
  }

  async function onPointerUp(_e: PointerEvent) {
    if (!isDragging || dragLayerId === null) return;

    const layer = project.layers.find((l) => l.id === dragLayerId);
    if (layer) {
      // Sync final position to backend
      await project.updateLayer(dragLayerId, { position: layer.position });
    }

    isDragging = false;
    dragLayerId = null;
  }
</script>

{#if project.metadata}
  <canvas
    bind:this={canvas}
    width={project.metadata.width}
    height={project.metadata.height}
    class="max-h-full max-w-full cursor-crosshair"
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
  ></canvas>
{:else}
  <p class="text-zinc-500">Open a GIF to get started</p>
{/if}
