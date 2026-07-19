import { convertFileSrc } from '@tauri-apps/api/core';
import { getFontData } from '$lib/commands';
import type { LayerInfo, Keyframe } from '$lib/types';

// Bounded LRU caches.  Maps preserve insertion order, so "least recently
// used" is the first key: reads re-insert to refresh recency and inserts
// beyond the cap evict the first entry.
const IMAGE_CACHE_CAP = 48;
const imageCache = new Map<string, HTMLImageElement>();

const TEXT_RASTER_CACHE_CAP = 16;
interface TextRaster {
  canvas: HTMLCanvasElement;
  pad: number;
}
const textRasterCache = new Map<string, TextRaster>();

function lruGet<K, V>(cache: Map<K, V>, key: K): V | undefined {
  const value = cache.get(key);
  if (value !== undefined) {
    // Re-insert to mark as most recently used.
    cache.delete(key);
    cache.set(key, value);
  }
  return value;
}

function lruSet<K, V>(cache: Map<K, V>, key: K, value: V, cap: number): void {
  cache.delete(key); // refresh recency if already present
  cache.set(key, value);
  if (cache.size > cap) {
    // Evict the least recently used entry (first key in insertion order).
    cache.delete(cache.keys().next().value as K);
  }
}

/** Test hook / project-close hygiene: drop all cached render assets. */
export function clearRenderCaches(): void {
  imageCache.clear();
  textRasterCache.clear();
}

/**
 * Map a layer font family to the bundled typeface actually used for
 * rasterisation, mirroring the backend substitution in
 * src-tauri/src/fonts.rs (impact|anton → Anton, everything else →
 * Liberation Sans, served by LiberationSans-Bold.ttf) so preview and
 * export draw the same glyphs.
 */
export function resolveFontFamily(family?: string | null): string {
  const f = (family ?? 'Impact').toLowerCase();
  // The Liberation Sans fallback also absorbs the legacy "Liberation Sans
  // Bold" label stored by projects saved before the family was renamed.
  return f === 'impact' || f === 'anton' ? 'Anton' : 'Liberation Sans';
}

function base64ToArrayBuffer(b64: string): ArrayBuffer {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

// Callbacks registered by consumers (Canvas.svelte) that want to repaint
// once a bundled FontFace finishes loading: any text raster produced before
// then was drawn with a fallback font and has just been evicted from the
// cache, so a re-render picks up the real typeface.
const fontsReadyCallbacks = new Set<() => void>();

/**
 * Register a callback invoked each time a bundled font finishes loading
 * (after the stale text rasters have been dropped). Returns an unsubscribe
 * function suitable for use as an $effect cleanup.
 */
export function onFontsReady(cb: () => void): () => void {
  fontsReadyCallbacks.add(cb);
  return () => fontsReadyCallbacks.delete(cb);
}

// Kick off loading of the bundled fonts at module init so canvas text is
// rasterised with them; the very first paint may fall back until loaded.
// The TTFs ship only inside the Rust binary (src-tauri/src/fonts.rs): the
// bytes are fetched over IPC (get_font_data) and registered via the JS
// FontFace API — there is no static/fonts copy and no CSS @font-face.
// Guarded so importing this module never throws in environments without
// the CSS Font Loading API or a Tauri backend (e.g. jsdom under vitest,
// where the invoke mock exists but FontFace does not).
if (
  typeof document !== 'undefined' &&
  'fonts' in document &&
  typeof FontFace !== 'undefined'
) {
  for (const family of ['Anton', 'Liberation Sans']) {
    try {
      getFontData(family)
        .then(async (b64) => {
          // A rejected/undefined IPC response lands in the catch below.
          const face = new FontFace(family, base64ToArrayBuffer(b64));
          await face.load();
          document.fonts.add(face);
          // Any text raster produced before this point was drawn with a
          // fallback font: drop them (image cache is unaffected) and let
          // registered consumers trigger a repaint.
          textRasterCache.clear();
          for (const cb of fontsReadyCallbacks) cb();
        })
        .catch((err) => {
          // First-paint fallback is acceptable; no toast, but leave a trace.
          console.warn(`Failed to load bundled font "${family}":`, err);
        });
    } catch (err) {
      // No Tauri IPC bridge (e.g. plain-browser dev or test runs).
      console.warn(`Failed to request bundled font "${family}":`, err);
    }
  }
}

async function loadImage(src: string): Promise<HTMLImageElement> {
  const cached = lruGet(imageCache, src);
  if (cached) return cached;
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      lruSet(imageCache, src, img, IMAGE_CACHE_CAP);
      resolve(img);
    };
    img.onerror = reject;
    img.src = src;
  });
}

/**
 * Cache key covering every field that affects the rasterised text image.
 * The layer id is deliberately excluded so identical text layers share a
 * single raster.
 *
 * Keep in sync with RenderCacheKey in src-tauri/src/text_renderer.rs.
 */
function textRasterKey(layer: LayerInfo): string {
  return JSON.stringify([
    layer.text ?? '',
    layer.font_family ?? null,
    layer.font_size ?? null,
    layer.color ?? null,
    layer.stroke ? [layer.stroke.width, layer.stroke.color] : null,
    layer.text_align ?? null,
    layer.max_width ?? null,
  ]);
}

export function wrapText(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
): string[] {
  const words = text.split(/\s+/);
  const lines: string[] = [];
  let current = '';

  for (const word of words) {
    const candidate = current ? `${current} ${word}` : word;
    if (ctx.measureText(candidate).width <= maxWidth || !current) {
      current = candidate;
    } else {
      if (current) lines.push(current);
      current = word;
    }
  }
  if (current) lines.push(current);
  return lines.length ? lines : [''];
}

export function interpolateKeyframes(
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

export async function renderFrame(
  ctx: CanvasRenderingContext2D,
  framePath: string,
  layers: LayerInfo[],
  frameIndex: number,
) {
  const { width, height } = ctx.canvas;
  ctx.clearRect(0, 0, width, height);

  // Draw base frame
  const frameSrc = convertFileSrc(framePath);
  const baseImg = await loadImage(frameSrc);
  ctx.drawImage(baseImg, 0, 0, width, height);

  // Draw layers in order (back to front)
  for (const layer of layers) {
    if (!layer.visible) continue;
    const [start, end] = layer.frame_range;
    if (frameIndex < start || frameIndex > end) continue;

    const interp = interpolateKeyframes(layer.keyframes, frameIndex);
    const [tx, ty] = interp ? interp.position : layer.position;
    const layerOpacity = interp ? interp.opacity : layer.opacity;

    ctx.save();
    ctx.globalAlpha = layerOpacity;
    // Build combined rotation × scale/skew matrix matching compositor.rs.
    // dst_x = a*src_x + c*src_y + tx
    // dst_y = b*src_x + d*src_y + ty
    const theta = (layer.rotation ?? 0) * (Math.PI / 180);
    const cosT = Math.cos(theta);
    const sinT = Math.sin(theta);
    const sx = layer.scale_x;
    const sy = layer.scale_y;
    const kx = layer.skew_x;
    const ky = layer.skew_y;
    const a = cosT * sx - sinT * ky;
    const b = sinT * sx + cosT * ky;
    const c = cosT * kx - sinT * sy;
    const d = sinT * kx + cosT * sy;
    ctx.transform(a, b, c, d, tx, ty);

    if (layer.layer_type === 'image') {
      if (!layer.source_path) { ctx.restore(); continue; }
      const img = await loadImage(convertFileSrc(layer.source_path));
      ctx.drawImage(img, 0, 0);
    } else if (layer.layer_type === 'text') {
      // Rasterising text is expensive (layout + stroke + fill on a fresh
      // offscreen canvas), so finished rasters are cached by content: on a
      // hit we skip layout entirely and just composite the cached image.
      const rasterKey = textRasterKey(layer);
      let raster = lruGet(textRasterCache, rasterKey);
      if (!raster) {
        const fontSize = layer.font_size ?? 48;
        const align = (layer.text_align ?? 'center') as CanvasTextAlign;
        const font = `${fontSize}px "${resolveFontFamily(layer.font_family)}", sans-serif`;
        ctx.font = font; // used for measurement below

        const text = layer.text ?? '';
        const lineHeight = fontSize * 1.2;
        const lines = layer.max_width ? wrapText(ctx, text, layer.max_width) : [text];
        const maxW = layer.max_width ?? ctx.measureText(text).width;

        // Rasterise stroke + fill at FULL alpha into an offscreen canvas and
        // composite it once at the layer opacity.  This matches the backend's
        // flattening: a translucent layer's stroke must not bleed through its
        // fill.  `pad` mirrors the backend's stroke_pad so stroke overflow is
        // not clipped; the offscreen image is drawn at (-pad, -pad) so the
        // glyph box origin stays on the transform origin.
        const pad = layer.stroke ? Math.ceil(layer.stroke.width) + 2 : 0;
        const off = document.createElement('canvas');
        off.width = Math.max(1, Math.ceil(maxW) + pad * 2);
        off.height = Math.max(1, Math.ceil(lines.length * lineHeight + fontSize * 0.5) + pad * 2);
        const octx = off.getContext('2d');
        if (octx) {
          octx.font = font;
          octx.textBaseline = 'top';
          octx.textAlign = 'left'; // we compute x manually

          lines.forEach((line, i) => {
            const lineW = octx.measureText(line).width;
            let x = 0;
            if (align === 'center') x = (maxW - lineW) / 2;
            else if (align === 'right') x = maxW - lineW;
            const y = i * lineHeight;

            if (layer.stroke) {
              const [sr, sg, sb, sa] = layer.stroke.color;
              octx.strokeStyle = `rgba(${sr},${sg},${sb},${sa / 255})`;
              octx.lineWidth = layer.stroke.width * 2;
              octx.lineJoin = 'round';
              octx.strokeText(line, x + pad, y + pad);
            }
            const [r, g, b, a] = layer.color ?? [255, 255, 255, 255];
            octx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
            octx.fillText(line, x + pad, y + pad);
          });

          raster = { canvas: off, pad };
          lruSet(textRasterCache, rasterKey, raster, TEXT_RASTER_CACHE_CAP);
        }
      }
      if (raster) ctx.drawImage(raster.canvas, -raster.pad, -raster.pad);
    } else if (layer.layer_type === 'flare') {
      // resetTransform() cancels the per-layer affine transform applied above;
      // flare elements must be drawn in canvas coordinates, not layer space.
      ctx.resetTransform();
      ctx.globalCompositeOperation = 'lighter';

      const [ox, oy] = interp ? interp.position : layer.position;
      const intensity = layer.intensity ?? 1.0;
      const scale = layer.scale ?? 1.0;
      const pulseSpeed = layer.pulse_speed ?? 0.15;
      const brightness = Math.min(
        2.0,
        intensity * (1.0 + 0.3 * Math.sin(frameIndex * pulseSpeed)),
      );

      // 1. Central white glow
      const glowRadius = scale * 80;
      const glowGrad = ctx.createRadialGradient(ox, oy, 0, ox, oy, glowRadius);
      glowGrad.addColorStop(0, `rgba(255,255,255,${brightness.toFixed(3)})`);
      glowGrad.addColorStop(0.3, `rgba(255,255,255,${(brightness * 0.7).toFixed(3)})`);
      glowGrad.addColorStop(1, 'rgba(255,255,255,0)');
      ctx.fillStyle = glowGrad as unknown as string;
      ctx.beginPath();
      ctx.arc(ox, oy, glowRadius, 0, Math.PI * 2);
      ctx.fill();

      // 2. Starburst — 8 streaks radiating from origin
      const streakLen = scale * 200;
      for (let i = 0; i < 8; i++) {
        const angle = (i * Math.PI) / 8;
        const streakAlpha = Math.min(1, brightness * 0.6);
        for (const dir of [1, -1] as const) {
          const ex = ox + Math.cos(angle) * streakLen * dir;
          const ey = oy + Math.sin(angle) * streakLen * dir;
          const grad = ctx.createLinearGradient(ox, oy, ex, ey);
          grad.addColorStop(0, `rgba(255,255,255,${streakAlpha.toFixed(3)})`);
          grad.addColorStop(1, 'rgba(255,255,255,0)');
          ctx.strokeStyle = grad as unknown as string;
          ctx.lineWidth = 1.5;
          ctx.beginPath();
          ctx.moveTo(ox, oy);
          ctx.lineTo(ex, ey);
          ctx.stroke();
        }
      }

      // 3. Yellow ring + orange outer halo
      const haloRadius = scale * 100;
      const haloThick = scale * 25;
      const haloGrad = ctx.createRadialGradient(
        ox, oy, haloRadius - haloThick,
        ox, oy, haloRadius + haloThick,
      );
      haloGrad.addColorStop(0, 'rgba(255,232,124,0)');
      // Ring peak matches the backend's #FFE87C yellow (flare_renderer.rs).
      haloGrad.addColorStop(0.5, `rgba(255,232,124,${Math.min(1, brightness * 0.5).toFixed(3)})`);
      haloGrad.addColorStop(1, 'rgba(255,232,124,0)');
      ctx.fillStyle = haloGrad as unknown as string;
      ctx.beginPath();
      ctx.arc(ox, oy, haloRadius + haloThick, 0, Math.PI * 2);
      ctx.fill();

      const outerGrad = ctx.createRadialGradient(ox, oy, scale * 80, ox, oy, scale * 140);
      outerGrad.addColorStop(0, 'rgba(255,123,0,0)');
      // Peak brightness matches the backend's brightness * 0.4 outer halo.
      outerGrad.addColorStop(0.6, `rgba(255,123,0,${Math.min(1, brightness * 0.4).toFixed(3)})`);
      outerGrad.addColorStop(1, 'rgba(255,123,0,0)');
      ctx.fillStyle = outerGrad as unknown as string;
      ctx.beginPath();
      ctx.arc(ox, oy, scale * 140, 0, Math.PI * 2);
      ctx.fill();

      // 4. Blue ghost artifacts along axis toward canvas centre
      const { width, height } = ctx.canvas;
      const axisX = width / 2 - ox;
      const axisY = height / 2 - oy;

      const ghostOffsets = [0.3, 0.6, 1.0, 1.4];
      const ghostSizes   = [0.3, 0.2, 0.4, 0.15];
      const ghostAlphas  = [0.6, 0.4, 0.7, 0.3];

      for (let i = 0; i < 4; i++) {
        const phase = i * 0.5;
        // No clamp: the backend leaves ghost brightness unclamped and lets
        // the rasteriser saturate, so >1 values must survive here too.
        const gb =
          brightness * ghostAlphas[i] * (1 + 0.3 * Math.sin(frameIndex * pulseSpeed + phase));
        const gx = ox + axisX * ghostOffsets[i];
        const gy = oy + axisY * ghostOffsets[i];
        const gr = scale * 80 * ghostSizes[i];
        if (gr < 0.5) continue;
        const ghostGrad = ctx.createRadialGradient(gx, gy, 0, gx, gy, gr);
        ghostGrad.addColorStop(0, `rgba(75,110,175,${gb.toFixed(3)})`);
        ghostGrad.addColorStop(1, 'rgba(75,110,175,0)');
        ctx.fillStyle = ghostGrad as unknown as string;
        ctx.beginPath();
        ctx.arc(gx, gy, gr, 0, Math.PI * 2);
        ctx.fill();
      }

      // Composite op is also reset by ctx.restore() below; this explicit reset
      // is defensive and makes the intent clear to future readers.
      ctx.globalCompositeOperation = 'source-over';
    }

    ctx.restore();
  }
}
