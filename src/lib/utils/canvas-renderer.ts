import { convertFileSrc } from '@tauri-apps/api/core';
import type { LayerInfo } from '$lib/types';

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

    const [tx, ty] = layer.position;

    ctx.save();
    ctx.globalAlpha = layer.opacity;
    // Apply affine: ctx.transform(a, b, c, d, e, f)
    // a=scale_x, b=skew_y, c=skew_x, d=scale_y, e=tx, f=ty
    ctx.transform(layer.scale_x, layer.skew_y, layer.skew_x, layer.scale_y, tx, ty);

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
