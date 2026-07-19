<script lang="ts">
  import { save } from '@tauri-apps/plugin-dialog';
  import { listen } from '@tauri-apps/api/event';
  import * as cmd from '$lib/commands';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import type { ExportFormat } from '$lib/types';

  let { open = false, onclose }: { open: boolean; onclose: () => void } = $props();

  let format = $state<ExportFormat>('Gif');
  let quality = $state(80);
  let exporting = $state(false);
  let progress = $state(0);
  let error = $state('');

  const stillFormats = new Set<ExportFormat>(['Png', 'Jpeg', 'WebP']);
  const isStill = $derived(stillFormats.has(format));
  // Quality is meaningful only for JPEG; PNG and WebP are lossless.
  const showQuality = $derived(format === 'Jpeg');

  $effect(() => {
    if (open) {
      cmd
        .checkFfmpeg()
        .then((available) => (ui.ffmpegAvailable = available))
        .catch(() => (ui.ffmpegAvailable = false));
    }
  });

  const formatExtensions: Record<ExportFormat, string> = {
    Gif: 'gif',
    Mp4: 'mp4',
    WebM: 'webm',
    Png: 'png',
    Jpeg: 'jpg',
    WebP: 'webp',
  };

  async function handleExport() {
    error = '';
    const ext = formatExtensions[format];
    const path = await save({
      filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
      defaultPath: `output.${ext}`,
    });
    if (!path) return;

    exporting = true;
    progress = 0;

    const unlisten = await listen<number>('export-progress', (event) => {
      progress = event.payload;
    });

    try {
      await cmd.exportProject(
        {
          format,
          quality,
          resize: null,
          frame_index: isStill ? ui.currentFrame : null,
        },
        path,
      );
      onclose();
    } catch (e) {
      error = `Export failed: ${e}`;
    } finally {
      exporting = false;
      unlisten();
    }
  }
</script>

{#if open}
  <!-- Backdrop -->
  <div class="fixed inset-0 z-40 bg-bed/70" role="presentation" onclick={onclose}></div>

  <!-- Dialog -->
  <div class="fixed left-1/2 top-1/2 z-50 w-80 -translate-x-1/2 -translate-y-1/2 rounded-lg border border-line bg-film p-6 shadow-xl">
    <h2 class="mb-4 font-display text-2xl uppercase leading-none tracking-wide text-ink">Export</h2>

    <div class="space-y-4">
      <label class="block text-sm">
        Format
        <select bind:value={format} class="mt-1 block w-full rounded bg-raise px-3 py-2 text-sm text-ink">
          <option value="Gif">GIF</option>
          <option value="Mp4" disabled={!ui.ffmpegAvailable}>MP4 {!ui.ffmpegAvailable ? '(ffmpeg required)' : ''}</option>
          <option value="WebM" disabled={!ui.ffmpegAvailable}>WebM {!ui.ffmpegAvailable ? '(ffmpeg required)' : ''}</option>
          <option value="Png">PNG (lossless)</option>
          <option value="Jpeg">JPEG</option>
          <option value="WebP">WebP (lossless)</option>
        </select>
      </label>

      {#if isStill}
        <p class="text-xs text-dim">Exports frame {ui.currentFrame + 1} as a still image.</p>
      {/if}

      {#if showQuality}
        <label class="block text-sm">
          Quality: {quality}
          <input type="range" min="1" max="100" bind:value={quality} class="mt-1 block w-full accent-lamp" />
        </label>
      {/if}

      {#if exporting && !isStill}
        {@const total = project.metadata?.frame_count ?? 1}
        {@const pct = Math.min(100, Math.round((progress / total) * 100))}
        <div class="h-2 rounded-full bg-raise">
          <div class="h-full rounded-full bg-flare transition-all motion-reduce:transition-none" style="width: {pct}%"></div>
        </div>
        <p class="text-center text-xs text-dim">{pct}% &mdash; frame {progress} / {total}</p>
      {/if}

      {#if error}
        <p class="text-sm text-red-400">{error}</p>
      {/if}

      <div class="flex gap-2">
        <button onclick={onclose} disabled={exporting}
          class="flex-1 rounded bg-raise px-3 py-2 text-sm text-ink hover:bg-line disabled:opacity-40">Cancel</button>
        <button onclick={handleExport} disabled={exporting}
          class="flex-1 rounded bg-flare px-3 py-2 text-sm font-semibold text-bed hover:brightness-110 disabled:opacity-40">
          {exporting ? 'Exporting...' : 'Export'}
        </button>
      </div>
    </div>
  </div>
{/if}
