<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import { openMediaFile } from '$lib/utils/open-media';

  let { onerror }: { onerror: (msg: string) => void } = $props();
  let showExport = $state(false);

  async function handleAddImage() {
    const path = await open({
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
    });
    if (!path) return;
    try {
      const layer = await project.addImageLayer(path);
      ui.selectLayer(layer.id);
    } catch (e) {
      onerror(`Failed to add image: ${e}`);
    }
  }

  async function handleAddText() {
    try {
      const layer = await project.addTextLayer('Your text here');
      ui.selectLayer(layer.id);
    } catch (e) {
      onerror(`Failed to add text: ${e}`);
    }
  }

  async function handleMemeText() {
    if (!project.metadata) return;
    const { width, height } = project.metadata;
    try {
      // Top text layer
      const top = await project.addTextLayer('TOP TEXT');
      await project.updateLayer(top.id, {
        position: [0, Math.round(height * 0.05)],
        text_align: 'center',
        max_width: width,
      });
      // Bottom text layer
      const bot = await project.addTextLayer('BOTTOM TEXT');
      await project.updateLayer(bot.id, {
        position: [0, Math.round(height * 0.88)],
        text_align: 'center',
        max_width: width,
      });
      ui.selectLayer(bot.id);
    } catch (e) {
      onerror(`Failed to add meme text: ${e}`);
    }
  }

  async function handleAddFlare() {
    if (!project.metadata) return;
    try {
      const layer = await project.addFlareLayer();
      ui.selectLayer(layer.id);
    } catch (e) {
      onerror(`Failed to add solar flare: ${e}`);
    }
  }

  async function handleUndo() {
    try { await project.undo(); } catch (e) { onerror(`Undo failed: ${e}`); }
  }
  async function handleRedo() {
    try { await project.redo(); } catch (e) { onerror(`Redo failed: ${e}`); }
  }
</script>

<div class="flex items-center gap-2 border-b border-line bg-film px-4 py-2">
  <span class="mr-2 select-none font-display text-lg leading-none tracking-wide text-ink" aria-hidden="true">
    GIF<span class="text-lamp">·</span>EDITOR
  </span>
  <button onclick={openMediaFile}
    class="rounded border border-line bg-raise px-3 py-1.5 text-sm font-medium text-ink hover:border-dim">
    Open file
  </button>
  <span class="mx-1 h-5 w-px bg-line" aria-hidden="true"></span>
  <button onclick={handleAddImage} disabled={!project.isOpen}
    class="rounded bg-raise px-3 py-1.5 text-sm font-medium text-ink hover:bg-line disabled:opacity-40">
    Add image
  </button>
  <button onclick={handleAddText} disabled={!project.isOpen}
    class="rounded bg-raise px-3 py-1.5 text-sm font-medium text-ink hover:bg-line disabled:opacity-40">
    Add text
  </button>
  <button onclick={handleMemeText} disabled={!project.isOpen}
    class="rounded bg-raise px-3 py-1.5 text-sm font-medium text-ink hover:bg-line disabled:opacity-40">
    Meme text
  </button>
  <button onclick={handleAddFlare} disabled={!project.isOpen}
    class="rounded bg-raise px-3 py-1.5 text-sm font-medium text-ink hover:bg-line disabled:opacity-40">
    <span class="text-lamp" aria-hidden="true">✳</span> Solar flare
  </button>
  <span class="mx-1 h-5 w-px bg-line" aria-hidden="true"></span>
  <button onclick={handleUndo} disabled={!project.isOpen}
    class="rounded bg-raise px-2.5 py-1.5 text-sm text-dim hover:bg-line hover:text-ink disabled:opacity-40"
    title="Undo (Ctrl+Z)">
    ↩
  </button>
  <button onclick={handleRedo} disabled={!project.isOpen}
    class="rounded bg-raise px-2.5 py-1.5 text-sm text-dim hover:bg-line hover:text-ink disabled:opacity-40"
    title="Redo (Ctrl+Y)">
    ↪
  </button>
  <div class="flex-1"></div>
  <label class="flex items-center gap-1.5 text-sm text-dim">
    <input type="checkbox" checked={ui.previewExport} onchange={() => ui.togglePreviewExport()}
      disabled={!project.isOpen} class="accent-lamp" />
    Preview export
  </label>
  <button onclick={() => (showExport = true)} disabled={!project.isOpen}
    class="rounded bg-flare px-4 py-1.5 text-sm font-semibold text-bed hover:brightness-110 disabled:opacity-40">
    Export
  </button>
</div>

<ExportDialog open={showExport} onclose={() => (showExport = false)} />
