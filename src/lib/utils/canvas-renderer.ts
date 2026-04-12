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
      const fontSize = layer.font_size ?? 48;
      const align = (layer.text_align ?? 'center') as CanvasTextAlign;
      ctx.font = `${fontSize}px "${layer.font_family ?? 'Anton'}", sans-serif`;
      ctx.textBaseline = 'top';
      ctx.textAlign = 'left'; // we compute x manually

      const text = layer.text ?? '';
      const lineHeight = fontSize * 1.2;
      const lines = layer.max_width ? wrapText(ctx, text, layer.max_width) : [text];
      const maxW = layer.max_width ?? ctx.measureText(text).width;

      lines.forEach((line, i) => {
        const lineW = ctx.measureText(line).width;
        let x = 0;
        if (align === 'center') x = (maxW - lineW) / 2;
        else if (align === 'right') x = maxW - lineW;
        const y = i * lineHeight;

        if (layer.stroke) {
          ctx.strokeStyle = `rgba(${layer.stroke.color.join(',')})`;
          ctx.lineWidth = layer.stroke.width * 2;
          ctx.lineJoin = 'round';
          ctx.strokeText(line, x, y);
        }
        const [r, g, b, a] = layer.color ?? [255, 255, 255, 255];
        ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
        ctx.fillText(line, x, y);
      });
    } else if (layer.layer_type === 'flare') {
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
      glowGrad.addColorStop(0.3, `rgba(255,255,220,${(brightness * 0.7).toFixed(3)})`);
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
      haloGrad.addColorStop(0.5, `rgba(255,123,0,${Math.min(1, brightness * 0.5).toFixed(3)})`);
      haloGrad.addColorStop(1, 'rgba(255,232,124,0)');
      ctx.fillStyle = haloGrad as unknown as string;
      ctx.beginPath();
      ctx.arc(ox, oy, haloRadius + haloThick, 0, Math.PI * 2);
      ctx.fill();

      const outerGrad = ctx.createRadialGradient(ox, oy, scale * 80, ox, oy, scale * 140);
      outerGrad.addColorStop(0, 'rgba(255,123,0,0)');
      outerGrad.addColorStop(0.6, `rgba(255,123,0,${Math.min(1, brightness * 0.2).toFixed(3)})`);
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
        const gb = Math.min(
          1.0,
          brightness * ghostAlphas[i] * (1 + 0.3 * Math.sin(frameIndex * pulseSpeed + phase)),
        );
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

      ctx.globalCompositeOperation = 'source-over';
    }

    ctx.restore();
  }
}
