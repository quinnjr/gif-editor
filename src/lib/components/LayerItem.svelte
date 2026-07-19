<script lang="ts" module>
  import { getAvailableFonts } from '$lib/commands';

  // Fonts are fetched once and shared by every LayerItem instance.
  let fontsPromise: Promise<string[]> | null = null;
  function loadFonts(): Promise<string[]> {
    fontsPromise ??= getAvailableFonts().catch((err) => {
      fontsPromise = null; // allow a retry on the next text layer render
      throw err;
    });
    return fontsPromise;
  }
</script>

<script lang="ts">
  import type { LayerInfo, Keyframe } from '$lib/types';
  import { project } from '$lib/stores/project.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { interpolateKeyframes } from '$lib/utils/canvas-renderer';

  let { layer, selected = false, onselect, canMoveUp = false, canMoveDown = false, onmoveup, onmovedown }: {
    layer: LayerInfo;
    selected: boolean;
    onselect: () => void;
    canMoveUp?: boolean;
    canMoveDown?: boolean;
    onmoveup?: () => void;
    onmovedown?: () => void;
  } = $props();

  let fonts = $state<string[]>([]);
  const currentFont = $derived(layer.font_family ?? 'Impact');

  $effect(() => {
    if (layer.layer_type === 'text') {
      loadFonts()
        .then((available) => (fonts = available))
        .catch((err) => ui.showError(`Failed to load fonts: ${err}`));
    }
  });

  async function toggleVisibility() {
    try {
      await project.updateLayer(layer.id, { visible: !layer.visible });
    } catch (err) {
      ui.showError(`Failed to toggle visibility: ${err}`);
    }
  }

  function upsertKeyframe(keyframes: Keyframe[], kf: Keyframe): Keyframe[] {
    const filtered = keyframes.filter((k) => k.frame !== kf.frame);
    return [...filtered, kf].sort((a, b) => a.frame - b.frame);
  }

  function opacityChanges(value: number): Partial<LayerInfo> {
    if (layer.keyframes.length > 0) {
      const frame = ui.currentFrame;
      const interp = interpolateKeyframes(layer.keyframes, frame);
      const pos = interp ? interp.position : layer.position;
      return {
        keyframes: upsertKeyframe(layer.keyframes, {
          frame,
          position: pos,
          opacity: value,
        }),
      };
    }
    return { opacity: value };
  }

  // Sliders live-update local state on `input` (no backend call, no history
  // entry) and persist ONE backend update on `change` (drag release), so a
  // single drag doesn't flood the 50-entry undo history.
  function handleOpacityInput(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    project.updateLayerLocal(layer.id, opacityChanges(value));
  }

  async function handleOpacityChange(e: Event) {
    const value = parseFloat((e.target as HTMLInputElement).value);
    try {
      await project.updateLayer(layer.id, opacityChanges(value));
    } catch (err) {
      ui.showError(`Failed to update opacity: ${err}`);
    }
  }

  function handleSliderInput(key: 'intensity' | 'scale' | 'pulse_speed') {
    return (e: Event) => {
      const value = parseFloat((e.target as HTMLInputElement).value);
      project.updateLayerLocal(layer.id, { [key]: value });
    };
  }

  function handleSliderChange(key: 'intensity' | 'scale' | 'pulse_speed') {
    return async (e: Event) => {
      const value = parseFloat((e.target as HTMLInputElement).value);
      try {
        await project.updateLayer(layer.id, { [key]: value });
      } catch (err) {
        ui.showError(`Failed to update ${key.replace('_', ' ')}: ${err}`);
      }
    };
  }

  async function handleRemove() {
    try {
      await project.removeLayer(layer.id);
    } catch (err) {
      ui.showError(`Failed to remove layer: ${err}`);
    }
  }
</script>

<div class="border-b border-line border-l-2 px-3 py-2 cursor-pointer {selected ? 'border-l-lamp bg-raise' : 'border-l-transparent hover:bg-raise/60'}"
  onclick={onselect} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && onselect()}>
  <div class="flex items-center gap-2">
    <button onclick={(e) => { e.stopPropagation(); toggleVisibility(); }}
      class="text-sm {layer.visible ? 'text-ink' : 'text-dim/60'}"
      title={layer.visible ? 'Hide layer' : 'Show layer'}>
      {layer.visible ? '👁' : '—'}
    </button>
    <span class="flex-1 truncate text-sm">{layer.name}</span>
    <span class="text-xs text-dim">{layer.layer_type}</span>
    <button onclick={(e) => { e.stopPropagation(); onmoveup?.(); }}
      disabled={!canMoveUp}
      class="text-xs text-dim hover:text-lamp disabled:cursor-default disabled:opacity-30 disabled:hover:text-dim"
      title="Move layer up">▲</button>
    <button onclick={(e) => { e.stopPropagation(); onmovedown?.(); }}
      disabled={!canMoveDown}
      class="text-xs text-dim hover:text-lamp disabled:cursor-default disabled:opacity-30 disabled:hover:text-dim"
      title="Move layer down">▼</button>
    <button onclick={(e) => { e.stopPropagation(); project.duplicateLayer(layer.id).catch((err) => ui.showError(`Failed to duplicate layer: ${err}`)); }}
      class="text-xs text-dim hover:text-lamp" title="Duplicate layer">⧉</button>
    <button onclick={(e) => { e.stopPropagation(); handleRemove(); }}
      class="text-xs text-dim hover:text-red-400" title="Remove layer">✕</button>
  </div>

  {#if selected}
    {@const interp = layer.keyframes.length > 0
      ? interpolateKeyframes(layer.keyframes, ui.currentFrame)
      : null}
    <div class="mt-2 space-y-2">
      <label class="flex items-center gap-2 text-xs text-dim">
        Opacity
        <input type="range" min="0" max="1" step="0.05" value={interp ? interp.opacity : layer.opacity}
          oninput={handleOpacityInput} onchange={handleOpacityChange} class="flex-1 accent-lamp" />
        <span class="w-8 text-right">{Math.round((interp ? interp.opacity : layer.opacity) * 100)}%</span>
      </label>
      <div class="text-xs text-dim">
        Frames: {layer.frame_range[0]} – {layer.frame_range[1]}
      </div>
      <div class="text-xs text-dim">
        Position: ({Math.round(layer.position[0])}, {Math.round(layer.position[1])})
      </div>
      {#if layer.layer_type !== 'flare'}
        <div class="flex gap-1">
          <button onclick={(e) => { e.stopPropagation(); project.flipLayer(layer.id, 'horizontal').catch((err) => ui.showError(`Failed to flip layer: ${err}`)); }}
            class="rounded bg-raise px-2 py-0.5 text-xs text-ink hover:bg-line">
            Flip H
          </button>
          <button onclick={(e) => { e.stopPropagation(); project.flipLayer(layer.id, 'vertical').catch((err) => ui.showError(`Failed to flip layer: ${err}`)); }}
            class="rounded bg-raise px-2 py-0.5 text-xs text-ink hover:bg-line">
            Flip V
          </button>
        </div>
      {/if}
      {#if layer.layer_type !== 'flare'}
        <label class="flex items-center gap-2 text-xs text-dim">
          Rotation
          <input type="number" step="1" value={layer.rotation}
            onchange={async (e) => {
              const deg = parseFloat((e.target as HTMLInputElement).value);
              try {
                await project.updateLayer(layer.id, { rotation: deg });
              } catch (err) {
                ui.showError(`Failed to update rotation: ${err}`);
              }
            }}
            class="w-16 rounded bg-raise px-1 py-0.5 text-xs text-ink" />
          °
          <button onclick={(e) => { e.stopPropagation(); project.updateLayer(layer.id, { rotation: (layer.rotation + 90) % 360 }).catch((err) => ui.showError(`Failed to update rotation: ${err}`)); }}
            class="rounded bg-raise px-1.5 py-0.5 text-xs text-ink hover:bg-line">+90</button>
          <button onclick={(e) => { e.stopPropagation(); project.updateLayer(layer.id, { rotation: ((layer.rotation - 90) + 360) % 360 }).catch((err) => ui.showError(`Failed to update rotation: ${err}`)); }}
            class="rounded bg-raise px-1.5 py-0.5 text-xs text-ink hover:bg-line">-90</button>
        </label>
      {/if}
      {#if layer.layer_type === 'flare'}
        <label class="flex items-center gap-2 text-xs text-dim">
          Intensity
          <input type="range" min="0" max="2" step="0.05"
            value={layer.intensity ?? 1}
            oninput={handleSliderInput('intensity')}
            onchange={handleSliderChange('intensity')}
            onpointerdown={(e) => e.stopPropagation()}
            class="flex-1 accent-flare" />
          <span class="w-10 text-right">{(layer.intensity ?? 1).toFixed(2)}</span>
        </label>
        <label class="flex items-center gap-2 text-xs text-dim">
          Scale
          <input type="range" min="0.25" max="4" step="0.05"
            value={layer.scale ?? 1}
            oninput={handleSliderInput('scale')}
            onchange={handleSliderChange('scale')}
            onpointerdown={(e) => e.stopPropagation()}
            class="flex-1 accent-flare" />
          <span class="w-10 text-right">{(layer.scale ?? 1).toFixed(2)}</span>
        </label>
        <label class="flex items-center gap-2 text-xs text-dim">
          Pulse
          <input type="range" min="0.05" max="0.5" step="0.01"
            value={layer.pulse_speed ?? 0.15}
            oninput={handleSliderInput('pulse_speed')}
            onchange={handleSliderChange('pulse_speed')}
            onpointerdown={(e) => e.stopPropagation()}
            class="flex-1 accent-flare" />
          <span class="w-10 text-right">{(layer.pulse_speed ?? 0.15).toFixed(2)}</span>
        </label>
      {/if}
      {#if layer.layer_type === 'text'}
        <label class="flex flex-col gap-1 text-xs text-dim">
          Text
          <textarea
            value={layer.text ?? ''}
            rows="3"
            onchange={async (e) => {
              try {
                await project.updateLayer(layer.id, { text: (e.target as HTMLTextAreaElement).value });
              } catch (err) {
                ui.showError(`Failed to update text: ${err}`);
              }
            }}
            onclick={(e) => e.stopPropagation()}
            class="w-full rounded bg-raise px-1 py-0.5 text-xs text-ink resize-none"></textarea>
        </label>
        <label class="flex items-center gap-2 text-xs text-dim">
          Font
          <select
            value={currentFont}
            onclick={(e) => e.stopPropagation()}
            onchange={async (e) => {
              try {
                await project.updateLayer(layer.id, { font_family: (e.target as HTMLSelectElement).value });
              } catch (err) {
                ui.showError(`Failed to update font: ${err}`);
              }
            }}
            class="flex-1 rounded bg-raise px-1 py-0.5 text-xs text-ink">
            {#if !fonts.includes(currentFont)}
              <option value={currentFont}>{currentFont}</option>
            {/if}
            {#each fonts as font (font)}
              <option value={font}>{font}</option>
            {/each}
          </select>
        </label>
        <div class="flex items-center gap-1 text-xs">
          <span class="text-dim">Align</span>
          {#each ['left', 'center', 'right'] as align (align)}
            <button
              onclick={(e) => { e.stopPropagation(); project.updateLayer(layer.id, { text_align: align }).catch((err) => ui.showError(`Failed to update alignment: ${err}`)); }}
              class="rounded px-1.5 py-0.5 text-xs {layer.text_align === align ? 'bg-lamp/15 text-lamp' : 'bg-raise text-dim hover:bg-line'}">
              {align[0].toUpperCase()}
            </button>
          {/each}
        </div>
        <label class="flex items-center gap-2 text-xs text-dim">
          Max width
          <input type="number" min="0" step="10"
            value={layer.max_width ?? ''}
            placeholder="none"
            onchange={async (e) => {
              const val = (e.target as HTMLInputElement).value;
              try {
                await project.updateLayer(layer.id, { max_width: val ? parseFloat(val) : null });
              } catch (err) {
                ui.showError(`Failed to update max width: ${err}`);
              }
            }}
            class="w-20 rounded bg-raise px-1 py-0.5 text-xs text-ink" />
          px
        </label>
      {/if}
    </div>
  {/if}
</div>
