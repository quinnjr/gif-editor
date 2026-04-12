<script lang="ts">
  import type { LayerInfo, Keyframe } from '$lib/types';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  let { layer, selected = false, onselect }: { layer: LayerInfo; selected: boolean; onselect: () => void } = $props();

  async function toggleVisibility() {
    await project.updateLayer(layer.id, { visible: !layer.visible });
  }

  function interpolateKeyframes(
    keyframes: Keyframe[],
    frameIndex: number,
  ): { position: [number, number]; opacity: number } | null {
    if (!keyframes || keyframes.length === 0) return null;
    if (frameIndex <= keyframes[0].frame) {
      return { position: keyframes[0].position, opacity: keyframes[0].opacity };
    }
    const last = keyframes[keyframes.length - 1];
    if (frameIndex >= last.frame) {
      return { position: last.position, opacity: last.opacity };
    }
    for (let i = 0; i < keyframes.length - 1; i++) {
      const a = keyframes[i];
      const b = keyframes[i + 1];
      if (frameIndex >= a.frame && frameIndex <= b.frame) {
        const span = b.frame - a.frame;
        const t = span > 0 ? (frameIndex - a.frame) / span : 0;
        return {
          position: [
            a.position[0] + t * (b.position[0] - a.position[0]),
            a.position[1] + t * (b.position[1] - a.position[1]),
          ],
          opacity: a.opacity + t * (b.opacity - a.opacity),
        };
      }
    }
    return { position: last.position, opacity: last.opacity };
  }

  function upsertKeyframe(keyframes: Keyframe[], kf: Keyframe): Keyframe[] {
    const filtered = keyframes.filter((k) => k.frame !== kf.frame);
    return [...filtered, kf].sort((a, b) => a.frame - b.frame);
  }

  async function handleOpacity(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    if (layer.keyframes.length > 0) {
      const frame = ui.currentFrame;
      const interp = interpolateKeyframes(layer.keyframes, frame);
      const pos = interp ? interp.position : layer.position;
      const newKfs = upsertKeyframe(layer.keyframes, {
        frame,
        position: pos,
        opacity: value,
      });
      await project.updateLayer(layer.id, { keyframes: newKfs });
    } else {
      await project.updateLayer(layer.id, { opacity: value });
    }
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
    <button onclick={(e) => { e.stopPropagation(); project.duplicateLayer(layer.id); }}
      class="text-xs text-zinc-400 hover:text-blue-400" title="Duplicate layer">⧉</button>
    <button onclick={(e) => { e.stopPropagation(); handleRemove(); }}
      class="text-xs text-zinc-400 hover:text-red-400" title="Remove layer">✕</button>
  </div>

  {#if selected}
    {@const interp = layer.keyframes.length > 0
      ? interpolateKeyframes(layer.keyframes, ui.currentFrame)
      : null}
    <div class="mt-2 space-y-2">
      <label class="flex items-center gap-2 text-xs text-zinc-300">
        Opacity
        <input type="range" min="0" max="1" step="0.05" value={interp ? interp.opacity : layer.opacity}
          oninput={handleOpacity} class="flex-1 accent-blue-500" />
        <span class="w-8 text-right">{Math.round((interp ? interp.opacity : layer.opacity) * 100)}%</span>
      </label>
      <div class="text-xs text-zinc-400">
        Frames: {layer.frame_range[0]} – {layer.frame_range[1]}
      </div>
      <div class="text-xs text-zinc-400">
        Position: ({Math.round(layer.position[0])}, {Math.round(layer.position[1])})
      </div>
      <div class="flex gap-1">
        <button onclick={(e) => { e.stopPropagation(); project.flipLayer(layer.id, 'horizontal'); }}
          class="rounded bg-zinc-600 px-2 py-0.5 text-xs text-white hover:bg-zinc-500">
          Flip H
        </button>
        <button onclick={(e) => { e.stopPropagation(); project.flipLayer(layer.id, 'vertical'); }}
          class="rounded bg-zinc-600 px-2 py-0.5 text-xs text-white hover:bg-zinc-500">
          Flip V
        </button>
      </div>
      <label class="flex items-center gap-2 text-xs text-zinc-300">
        Rotation
        <input type="number" step="1" value={layer.rotation}
          onchange={async (e) => {
            const deg = parseFloat((e.target as HTMLInputElement).value);
            await project.updateLayer(layer.id, { rotation: deg });
          }}
          class="w-16 rounded bg-zinc-700 px-1 py-0.5 text-xs text-white" />
        °
        <button onclick={(e) => { e.stopPropagation(); project.updateLayer(layer.id, { rotation: (layer.rotation + 90) % 360 }); }}
          class="rounded bg-zinc-600 px-1.5 py-0.5 text-xs text-white hover:bg-zinc-500">+90</button>
        <button onclick={(e) => { e.stopPropagation(); project.updateLayer(layer.id, { rotation: ((layer.rotation - 90) + 360) % 360 }); }}
          class="rounded bg-zinc-600 px-1.5 py-0.5 text-xs text-white hover:bg-zinc-500">-90</button>
      </label>
      {#if layer.layer_type === 'flare'}
        <label class="flex items-center gap-2 text-xs text-zinc-300">
          Intensity
          <input type="range" min="0" max="2" step="0.05"
            value={layer.intensity ?? 1}
            oninput={async (e) => {
              await project.updateLayer(layer.id, { intensity: parseFloat((e.target as HTMLInputElement).value) });
            }}
            onpointerdown={(e) => e.stopPropagation()}
            class="flex-1 accent-orange-500" />
          <span class="w-10 text-right">{(layer.intensity ?? 1).toFixed(2)}</span>
        </label>
        <label class="flex items-center gap-2 text-xs text-zinc-300">
          Scale
          <input type="range" min="0.25" max="4" step="0.05"
            value={layer.scale ?? 1}
            oninput={async (e) => {
              await project.updateLayer(layer.id, { scale: parseFloat((e.target as HTMLInputElement).value) });
            }}
            onpointerdown={(e) => e.stopPropagation()}
            class="flex-1 accent-orange-500" />
          <span class="w-10 text-right">{(layer.scale ?? 1).toFixed(2)}</span>
        </label>
        <label class="flex items-center gap-2 text-xs text-zinc-300">
          Pulse
          <input type="range" min="0.05" max="0.5" step="0.01"
            value={layer.pulse_speed ?? 0.15}
            oninput={async (e) => {
              await project.updateLayer(layer.id, { pulse_speed: parseFloat((e.target as HTMLInputElement).value) });
            }}
            onpointerdown={(e) => e.stopPropagation()}
            class="flex-1 accent-orange-500" />
          <span class="w-10 text-right">{(layer.pulse_speed ?? 0.15).toFixed(2)}</span>
        </label>
      {/if}
      {#if layer.layer_type === 'text'}
        <div class="flex items-center gap-1 text-xs">
          <span class="text-zinc-400">Align</span>
          {#each ['left', 'center', 'right'] as align (align)}
            <button
              onclick={(e) => { e.stopPropagation(); project.updateLayer(layer.id, { text_align: align }); }}
              class="rounded px-1.5 py-0.5 text-xs {layer.text_align === align ? 'bg-blue-600 text-white' : 'bg-zinc-600 text-zinc-300 hover:bg-zinc-500'}">
              {align[0].toUpperCase()}
            </button>
          {/each}
        </div>
        <label class="flex items-center gap-2 text-xs text-zinc-300">
          Max width
          <input type="number" min="0" step="10"
            value={layer.max_width ?? ''}
            placeholder="none"
            onchange={async (e) => {
              const val = (e.target as HTMLInputElement).value;
              await project.updateLayer(layer.id, { max_width: val ? parseFloat(val) : null });
            }}
            class="w-20 rounded bg-zinc-700 px-1 py-0.5 text-xs text-white" />
          px
        </label>
      {/if}
    </div>
  {/if}
</div>
