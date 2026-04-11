import { convertFileSrc } from '@tauri-apps/api/core';
import type { LayerInfo, Keyframe } from '$lib/types';

const imageCache = new Map<string, HTMLImageElement>();

async function loadImage(src: string): Promise<HTMLImageElement> {
  if (imageCache.has(src)) return imageCache.get(src)!;
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => { imageCache.set(src, img); resolve(img); };
    img.onerror = reject;
    img.src = src;
  });
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
      const fontSize = layer.font_size ?? 48;
      ctx.font = `${fontSize}px "${layer.font_family ?? 'Impact'}", sans-serif`;
      ctx.textBaseline = 'top';
      if (layer.stroke) {
        ctx.strokeStyle = `rgba(${layer.stroke.color.join(',')})`;
        ctx.lineWidth = layer.stroke.width * 2;
        ctx.lineJoin = 'round';
        ctx.strokeText(layer.text ?? '', 0, 0);
      }
      const [r, g, b, a] = layer.color ?? [255, 255, 255, 255];
      ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
      ctx.fillText(layer.text ?? '', 0, 0);
    }

    ctx.restore();
  }
}
