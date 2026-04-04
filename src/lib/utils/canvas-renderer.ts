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

    ctx.globalAlpha = layer.opacity;

    if (layer.layer_type === 'image') {
      if (!layer.source_path) continue;
      const img = await loadImage(convertFileSrc(layer.source_path));
      const w = (layer.source_width ?? img.naturalWidth) * layer.scale;
      const h = (layer.source_height ?? img.naturalHeight) * layer.scale;
      ctx.drawImage(img, layer.position[0], layer.position[1], w, h);
    } else if (layer.layer_type === 'text') {
      const fontSize = (layer.font_size ?? 48) * layer.scale;
      ctx.font = `${fontSize}px "${layer.font_family ?? 'Impact'}", sans-serif`;
      ctx.textBaseline = 'top';
      if (layer.stroke) {
        ctx.strokeStyle = `rgba(${layer.stroke.color.join(',')})`;
        ctx.lineWidth = layer.stroke.width * 2;
        ctx.lineJoin = 'round';
        ctx.strokeText(layer.text ?? '', layer.position[0], layer.position[1]);
      }
      const [r, g, b, a] = layer.color ?? [255, 255, 255, 255];
      ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
      ctx.fillText(layer.text ?? '', layer.position[0], layer.position[1]);
    }

    ctx.globalAlpha = 1;
  }
}
