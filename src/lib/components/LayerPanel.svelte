<script lang="ts">
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import LayerItem from './LayerItem.svelte';

  // Move a layer one step within the stack. `project.layers` is ordered
  // back-to-front (last = topmost), but the panel renders the list reversed
  // (topmost first), so "visually up" means a HIGHER index in the store.
  async function moveLayer(id: string, dir: 1 | -1) {
    const ids = project.layers.map((l) => l.id);
    const idx = ids.indexOf(id);
    const target = idx + dir;
    if (idx === -1 || target < 0 || target >= ids.length) return;
    [ids[idx], ids[target]] = [ids[target], ids[idx]];
    try {
      await project.reorderLayers(ids);
    } catch (e) {
      ui.showError(`Failed to reorder layers: ${e}`);
    }
  }
</script>

<div class="flex h-full flex-col">
  <div class="border-b border-zinc-700 px-3 py-2 text-sm font-medium text-zinc-300">Layers</div>
  <div class="flex-1 overflow-y-auto">
    {#if project.layers.length === 0}
      <p class="p-4 text-xs text-zinc-500">No layers yet. Add an image or text overlay.</p>
    {:else}
      {#each [...project.layers].reverse() as layer, i (layer.id)}
        <LayerItem {layer} selected={ui.selectedLayerId === layer.id}
          onselect={() => ui.selectLayer(layer.id)}
          canMoveUp={i > 0}
          canMoveDown={i < project.layers.length - 1}
          onmoveup={() => moveLayer(layer.id, 1)}
          onmovedown={() => moveLayer(layer.id, -1)} />
      {/each}
    {/if}
  </div>
</div>
