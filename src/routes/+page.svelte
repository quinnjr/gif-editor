<script lang="ts">
  import Toolbar from '$lib/components/Toolbar.svelte';
  import Toast from '$lib/components/Toast.svelte';

  let toastMessage = $state('');
  let toastType = $state<'error' | 'success'>('error');

  function showError(msg: string) {
    toastMessage = msg;
    toastType = 'error';
  }
</script>

<div class="flex h-screen flex-col bg-zinc-900 text-white">
  <Toolbar onerror={showError} />

  <div class="flex flex-1 overflow-hidden">
    <!-- Canvas area -->
    <div class="flex-1 flex items-center justify-center bg-zinc-950">
      <p class="text-zinc-500">Open a GIF to get started</p>
    </div>
    <!-- Layer panel -->
    <div class="w-64 border-l border-zinc-700 bg-zinc-800">
      <p class="p-4 text-sm text-zinc-400">Layers</p>
    </div>
  </div>

  <!-- Timeline -->
  <div class="h-24 border-t border-zinc-700 bg-zinc-800">
    <p class="p-4 text-sm text-zinc-400">Timeline</p>
  </div>
</div>

{#if toastMessage}
  <Toast message={toastMessage} type={toastType} />
{/if}
