<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import Toast from '$lib/components/Toast.svelte';
  import Canvas from '$lib/components/Canvas.svelte';
  import LayerPanel from '$lib/components/LayerPanel.svelte';
  import Timeline from '$lib/components/Timeline.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  function showError(msg: string) {
    ui.showError(msg);
  }

  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    return (
      target.tagName === 'INPUT' ||
      target.tagName === 'TEXTAREA' ||
      target.tagName === 'SELECT' ||
      target.isContentEditable
    );
  }

  // All app-global shortcuts live in this one handler so every binding gets
  // the same guards (project open, not typing in a field).
  function handleKeyDown(e: KeyboardEvent) {
    if (!project.isOpen) return;
    if (!(e.ctrlKey || e.metaKey)) return;
    if (isEditableTarget(e.target)) return;

    // Shift+key produces the shifted character in e.key on US layouts
    // ('=' -> '+', '-' -> '_', 'z' -> 'Z'), so match both forms.
    if (e.shiftKey && (e.key === '=' || e.key === '+')) {
      // Ctrl/Cmd + Shift + = : Scale all layers up by 10%
      e.preventDefault();
      project.scaleAllLayers(1.1, 1.1).catch((err) => ui.showError(`Failed to scale layers: ${err}`));
    } else if (e.shiftKey && (e.key === '-' || e.key === '_')) {
      // Ctrl/Cmd + Shift + - : Scale all layers down by 10%
      e.preventDefault();
      project.scaleAllLayers(0.9, 0.9).catch((err) => ui.showError(`Failed to scale layers: ${err}`));
    } else if (e.key.toLowerCase() === 'z' && !e.shiftKey) {
      // Ctrl/Cmd + Z : undo
      e.preventDefault();
      project.undo().catch((err) => ui.showError(`Undo failed: ${err}`));
    } else if (e.key === 'y' || (e.key.toLowerCase() === 'z' && e.shiftKey)) {
      // Ctrl/Cmd + Y or Ctrl/Cmd + Shift + Z : redo
      e.preventDefault();
      project.redo().catch((err) => ui.showError(`Redo failed: ${err}`));
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="flex h-screen flex-col bg-zinc-900 text-white">
  <Toolbar onerror={showError} />

  <div class="flex flex-1 overflow-hidden">
    <!-- Canvas area -->
    <div class="flex-1 flex items-center justify-center bg-zinc-950 overflow-hidden p-4">
      <Canvas />
    </div>
    <!-- Layer panel -->
    <div class="w-64 border-l border-zinc-700 bg-zinc-800">
      <LayerPanel />
    </div>
  </div>

  <!-- Timeline -->
  <div class="h-24 border-t border-zinc-700 bg-zinc-800">
    <Timeline />
  </div>
</div>

{#if ui.toastMessage}
  <Toast message={ui.toastMessage} type={ui.toastType} nonce={ui.toastId} />
{/if}
