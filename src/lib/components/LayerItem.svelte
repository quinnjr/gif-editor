<script lang="ts">
  import type { LayerInfo } from '$lib/types';
  import { project } from '$lib/stores/project.svelte';

  let { layer, selected = false, onselect }: { layer: LayerInfo; selected: boolean; onselect: () => void } = $props();

  async function toggleVisibility() {
    await project.updateLayer(layer.id, { visible: !layer.visible });
  }

  async function handleOpacity(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    await project.updateLayer(layer.id, { opacity: value });
  }

  async function handleRemove() {
    await project.removeLayer(layer.id);
  }
</script>

<div class="border-b border-zinc-700 px-3 py-2 cursor-pointer {selected ? 'bg-zinc-600' : 'hover:bg-zinc-700'}"
  onclick={onselect} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && onselect()}>
  <div class="flex items-center gap-2">
    <button onclick={(e) => { e.stopPropagation(); toggleVisibility(); }}
      class="text-sm {layer.visible ? 'text-white' : 'text-zinc-500'}"
      title={layer.visible ? 'Hide layer' : 'Show layer'}>
      {layer.visible ? '👁' : '—'}
    </button>
    <span class="flex-1 truncate text-sm">{layer.name}</span>
    <span class="text-xs text-zinc-400">{layer.layer_type}</span>
    <button onclick={(e) => { e.stopPropagation(); handleRemove(); }}
      class="text-xs text-zinc-400 hover:text-red-400" title="Remove layer">✕</button>
  </div>

  {#if selected}
    <div class="mt-2 space-y-2">
      <label class="flex items-center gap-2 text-xs text-zinc-300">
        Opacity
        <input type="range" min="0" max="1" step="0.05" value={layer.opacity}
          oninput={handleOpacity} class="flex-1 accent-blue-500" />
        <span class="w-8 text-right">{Math.round(layer.opacity * 100)}%</span>
      </label>
      <div class="text-xs text-zinc-400">
        Frames: {layer.frame_range[0]} – {layer.frame_range[1]}
      </div>
      <div class="text-xs text-zinc-400">
        Position: ({Math.round(layer.position[0])}, {Math.round(layer.position[1])})
      </div>
    </div>
  {/if}
</div>
