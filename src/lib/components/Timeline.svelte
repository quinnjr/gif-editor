<script lang="ts">
  import { onDestroy } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  // Thumbnail URLs indexed by frame number
  let thumbnails = $state<string[]>([]);

  // Frame range drag state
  let dragging = $state<'start' | 'end' | null>(null);
  let stripEl = $state<HTMLDivElement | null>(null);

  // Derived: selected layer
  const selectedLayer = $derived(
    ui.selectedLayerId ? project.layers.find((l) => l.id === ui.selectedLayerId) ?? null : null,
  );

  // Load thumbnails whenever metadata changes
  $effect(() => {
    const meta = project.metadata;
    if (!meta) {
      thumbnails = [];
      return;
    }
    const count = meta.frame_count;
    thumbnails = new Array(count).fill('');
    for (let i = 0; i < count; i++) {
      project.getFramePath(i).then((path) => {
        thumbnails[i] = convertFileSrc(path);
      });
    }
  });

  // Playback timer using $effect + setTimeout chain
  $effect(() => {
    if (!ui.isPlaying || !project.metadata) return;

    let cancelled = false;
    const meta = project.metadata;

    function schedule() {
      if (cancelled) return;
      const frameIndex = ui.currentFrame;
      const delayMs = (meta.delays[frameIndex] ?? 100) / ui.playbackSpeed;
      const timer = setTimeout(() => {
        if (cancelled) return;
        const next = (ui.currentFrame + 1) % meta.frame_count;
        ui.setFrame(next);
        schedule();
      }, delayMs);
      return timer;
    }

    const timer = schedule();

    return () => {
      cancelled = true;
      if (timer !== undefined) clearTimeout(timer);
    };
  });

  function stepBackward() {
    if (!project.metadata) return;
    const prev = (ui.currentFrame - 1 + project.metadata.frame_count) % project.metadata.frame_count;
    ui.setFrame(prev);
  }

  function stepForward() {
    if (!project.metadata) return;
    const next = (ui.currentFrame + 1) % project.metadata.frame_count;
    ui.setFrame(next);
  }

  // Frame range overlay — percentage positions
  const rangeStartPct = $derived(() => {
    if (!selectedLayer || !project.metadata) return 0;
    return (selectedLayer.frame_range[0] / project.metadata.frame_count) * 100;
  });

  const rangeEndPct = $derived(() => {
    if (!selectedLayer || !project.metadata) return 100;
    return ((selectedLayer.frame_range[1] + 1) / project.metadata.frame_count) * 100;
  });

  function getFrameFromX(clientX: number): number {
    if (!stripEl || !project.metadata) return 0;
    const rect = stripEl.getBoundingClientRect();
    const pct = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return Math.round(pct * (project.metadata.frame_count - 1));
  }

  function onDragStart(handle: 'start' | 'end', e: PointerEvent) {
    dragging = handle;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onDragMove(e: PointerEvent) {
    if (!dragging || !selectedLayer || !project.metadata) return;
    const frame = getFrameFromX(e.clientX);
    const [start, end] = selectedLayer.frame_range;
    if (dragging === 'start') {
      const newStart = Math.min(frame, end);
      project.updateLayer(selectedLayer.id, { frame_range: [newStart, end] });
    } else {
      const newEnd = Math.max(frame, start);
      project.updateLayer(selectedLayer.id, { frame_range: [start, newEnd] });
    }
  }

  function onDragEnd(_e: PointerEvent) {
    dragging = null;
  }
</script>

{#if !project.metadata}
  <div class="flex h-full items-center justify-center text-sm text-zinc-400">
    Open a GIF to see the timeline
  </div>
{:else}
  <div class="flex h-full flex-col">
    <!-- Controls row -->
    <div class="flex items-center gap-3 border-b border-zinc-700 px-3 py-1 text-sm">
      <button
        onclick={stepBackward}
        class="rounded px-1.5 py-0.5 hover:bg-zinc-700 disabled:opacity-40"
        title="Previous frame"
        disabled={ui.isPlaying}
      >
        &#9664;
      </button>

      <button
        onclick={() => ui.togglePlayback()}
        class="rounded px-1.5 py-0.5 hover:bg-zinc-700"
        title={ui.isPlaying ? 'Pause' : 'Play'}
      >
        {#if ui.isPlaying}
          &#9646;&#9646;
        {:else}
          &#9654;
        {/if}
      </button>

      <button
        onclick={stepForward}
        class="rounded px-1.5 py-0.5 hover:bg-zinc-700 disabled:opacity-40"
        title="Next frame"
        disabled={ui.isPlaying}
      >
        &#9654;&#9654;
      </button>

      <span class="text-zinc-400">
        {ui.currentFrame + 1}&nbsp;/&nbsp;{project.metadata.frame_count}
      </span>

      <div class="flex-1"></div>

      <label class="flex items-center gap-1 text-zinc-300">
        Speed
        <select
          value={ui.playbackSpeed}
          onchange={(e) => ui.setPlaybackSpeed(parseFloat((e.target as HTMLSelectElement).value))}
          class="rounded bg-zinc-700 px-1 py-0.5 text-xs text-white"
        >
          <option value={0.25}>0.25x</option>
          <option value={0.5}>0.5x</option>
          <option value={1}>1x</option>
          <option value={2}>2x</option>
        </select>
      </label>
    </div>

    <!-- Thumbnail strip -->
    <div class="relative flex-1 overflow-hidden">
      <div
        bind:this={stripEl}
        class="flex h-full items-center gap-0.5 overflow-x-auto px-1"
        style="scrollbar-width: thin; scrollbar-color: #52525b transparent;"
      >
        {#each thumbnails as src, i}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="relative h-12 w-16 shrink-0 cursor-pointer overflow-hidden rounded border-2 {i === ui.currentFrame ? 'border-blue-400' : 'border-zinc-600'}"
            onclick={() => ui.setFrame(i)}
          >
            {#if src}
              <img {src} alt="Frame {i + 1}" class="h-full w-full object-cover" />
            {:else}
              <div class="h-full w-full bg-zinc-700"></div>
            {/if}
            <span class="absolute bottom-0 left-0 right-0 bg-black/50 text-center text-[9px] leading-3 text-zinc-300">
              {i + 1}
            </span>
          </div>
        {/each}
      </div>

      <!-- Frame range overlay -->
      {#if selectedLayer && project.metadata}
        {@const startPct = rangeStartPct()}
        {@const endPct = rangeEndPct()}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="pointer-events-none absolute inset-0 top-0"
          onpointermove={onDragMove}
          onpointerup={onDragEnd}
        >
          <!-- Left darkened region -->
          <div
            class="absolute inset-y-0 left-0 bg-black/40"
            style="width: {startPct}%"
          ></div>
          <!-- Right darkened region -->
          <div
            class="absolute inset-y-0 right-0 bg-black/40"
            style="left: {endPct}%"
          ></div>
          <!-- Range bar outline -->
          <div
            class="absolute inset-y-1 border-2 border-blue-500/70 bg-blue-500/10"
            style="left: {startPct}%; width: {endPct - startPct}%"
          ></div>
        </div>

        <!-- Drag handles (pointer-events enabled) -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="absolute inset-y-0 w-2 cursor-ew-resize bg-blue-500 opacity-80 hover:opacity-100"
          style="left: calc({startPct}% - 4px)"
          onpointerdown={(e) => onDragStart('start', e)}
          onpointermove={onDragMove}
          onpointerup={onDragEnd}
          role="slider"
          aria-label="Frame range start"
          aria-valuenow={selectedLayer.frame_range[0]}
          aria-valuemin={0}
          aria-valuemax={selectedLayer.frame_range[1]}
          tabindex="0"
        ></div>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="absolute inset-y-0 w-2 cursor-ew-resize bg-blue-500 opacity-80 hover:opacity-100"
          style="left: calc({endPct}% - 4px)"
          onpointerdown={(e) => onDragStart('end', e)}
          onpointermove={onDragMove}
          onpointerup={onDragEnd}
          role="slider"
          aria-label="Frame range end"
          aria-valuenow={selectedLayer.frame_range[1]}
          aria-valuemin={selectedLayer.frame_range[0]}
          aria-valuemax={project.metadata.frame_count - 1}
          tabindex="0"
        ></div>
      {/if}
    </div>
  </div>
{/if}
