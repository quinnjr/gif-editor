<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import ExportDialog from './ExportDialog.svelte';

  let { onerror }: { onerror: (msg: string) => void } = $props();
  let showExport = $state(false);

  async function handleOpenFile() {
    const path = await open({
      filters: [
        { name: 'Supported Media', extensions: ['gif', 'mp4', 'webm', 'png', 'jpg', 'jpeg', 'webp'] },
      ],
    });
    if (!path) return;
    try {
      await project.open(path);
      ui.setFrame(0);
    } catch (e) {
      onerror(`Failed to open file: ${e}`);
    }
  }

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
</script>

<div class="flex items-center gap-2 border-b border-zinc-700 bg-zinc-800 px-4 py-2">
  <button onclick={handleOpenFile}
    class="rounded bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-500">
    Open File
  </button>
  <button onclick={handleAddImage} disabled={!project.isOpen}
    class="rounded bg-zinc-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-500 disabled:opacity-40">
    Add Image
  </button>
  <button onclick={handleAddText} disabled={!project.isOpen}
    class="rounded bg-zinc-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-500 disabled:opacity-40">
    Add Text
  </button>
  <div class="flex-1"></div>
  <label class="flex items-center gap-1.5 text-sm text-zinc-300">
    <input type="checkbox" checked={ui.previewExport} onchange={() => ui.togglePreviewExport()}
      disabled={!project.isOpen} class="accent-blue-500" />
    Preview Export
  </label>
  <button onclick={() => (showExport = true)} disabled={!project.isOpen}
    class="rounded bg-green-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-green-500 disabled:opacity-40">
    Export
  </button>
</div>

<ExportDialog open={showExport} onclose={() => (showExport = false)} />
