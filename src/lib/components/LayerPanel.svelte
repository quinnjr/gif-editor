<script lang="ts">
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import LayerItem from './LayerItem.svelte';
</script>

<div class="flex h-full flex-col">
  <div class="border-b border-zinc-700 px-3 py-2 text-sm font-medium text-zinc-300">Layers</div>
  <div class="flex-1 overflow-y-auto">
    {#if project.layers.length === 0}
      <p class="p-4 text-xs text-zinc-500">No layers yet. Add an image or text overlay.</p>
    {:else}
      {#each [...project.layers].reverse() as layer (layer.id)}
        <LayerItem {layer} selected={ui.selectedLayerId === layer.id}
          onselect={() => ui.selectLayer(layer.id)} />
      {/each}
    {/if}
  </div>
</div>
