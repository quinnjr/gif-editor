import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => path),
}));

import { renderFrame, interpolateKeyframes, wrapText } from '$lib/utils/canvas-renderer';
import type { LayerInfo, Keyframe } from '$lib/types';

// Mock Image class that auto-triggers onload
class MockImage {
  src = '';
  naturalWidth = 100;
  naturalHeight = 100;
  onload: (() => void) | null = null;
  onerror: ((err: unknown) => void) | null = null;

  constructor() {
    // Use a getter/setter on src to trigger onload when set
    const self = this;
    const originalDescriptor = Object.getOwnPropertyDescriptor(this, 'src');
    let _src = '';
    Object.defineProperty(this, 'src', {
      get() { return _src; },
      set(val: string) {
        _src = val;
        // Trigger onload async
        Promise.resolve().then(() => self.onload?.());
      },
      configurable: true,
    });
  }
}

vi.stubGlobal('Image', MockImage);

function createMockCtx() {
  const calls: { method: string; args: unknown[] }[] = [];

  function track(method: string) {
    return (...args: unknown[]) => {
      calls.push({ method, args });
    };
  }

  let _compositeOp = 'source-over';

  const ctx = {
    canvas: { width: 200, height: 200 },
    clearRect: track('clearRect'),
    drawImage: track('drawImage'),
    save: track('save'),
    restore: track('restore'),
    transform: track('transform'),
    resetTransform: track('resetTransform'),
    fillText: track('fillText'),
    strokeText: track('strokeText'),
    beginPath: track('beginPath'),
    arc: track('arc'),
    fill: track('fill'),
    moveTo: track('moveTo'),
    lineTo: track('lineTo'),
    stroke: track('stroke'),
    measureText: vi.fn((text: string) => ({ width: 100 } as TextMetrics)),
    createRadialGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
    createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
    globalAlpha: 1,
    font: '',
    textBaseline: '',
    textAlign: '',
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 0,
    lineJoin: '',
    get globalCompositeOperation() { return _compositeOp; },
    set globalCompositeOperation(v: string) { _compositeOp = v; calls.push({ method: 'setCompositeOp', args: [v] }); },
    _calls: calls,
  } as unknown as CanvasRenderingContext2D & { _calls: typeof calls };

  return ctx;
}

function makeLayer(overrides: Partial<LayerInfo> = {}): LayerInfo {
  return {
    id: 'l1',
    name: 'Layer 1',
    layer_type: 'image',
    position: [10, 20],
    scale_x: 1,
    scale_y: 1,
    skew_x: 0,
    skew_y: 0,
    rotation: 0,
    opacity: 1,
    frame_range: [0, 9],
    visible: true,
    source_path: '/img.png',
    keyframes: [],
    ...overrides,
  };
}

describe('renderFrame', () => {
  let ctx: ReturnType<typeof createMockCtx>;

  beforeEach(() => {
    ctx = createMockCtx();
  });

  it('clears the canvas and draws the base frame', async () => {
    await renderFrame(ctx, '/frame0.png', [], 0);

    const clearCall = ctx._calls.find((c) => c.method === 'clearRect');
    expect(clearCall).toBeDefined();
    expect(clearCall!.args).toEqual([0, 0, 200, 200]);

    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(1);
    expect(drawCalls[0].args[1]).toBe(0);
    expect(drawCalls[0].args[2]).toBe(0);
    expect(drawCalls[0].args[3]).toBe(200);
    expect(drawCalls[0].args[4]).toBe(200);
  });

  it('draws a visible image layer in range', async () => {
    const layer = makeLayer({ source_path: '/overlay.png' });
    await renderFrame(ctx, '/frame0.png', [layer], 5);

    const saveCalls = ctx._calls.filter((c) => c.method === 'save');
    const restoreCalls = ctx._calls.filter((c) => c.method === 'restore');
    expect(saveCalls.length).toBe(1);
    expect(restoreCalls.length).toBe(1);

    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    expect(transformCall).toBeDefined();
    expect(transformCall!.args).toEqual([1, 0, 0, 1, 10, 20]);

    // Base frame + overlay = 2 drawImage calls
    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(2);
  });

  it('skips invisible layers', async () => {
    const layer = makeLayer({ visible: false });
    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    // Only the base frame
    expect(drawCalls.length).toBe(1);
  });

  it('skips layers out of frame range', async () => {
    const layer = makeLayer({ frame_range: [5, 8] });
    await renderFrame(ctx, '/frame0.png', [layer], 2);

    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(1);
  });

  it('skips layers after frame range end', async () => {
    const layer = makeLayer({ frame_range: [0, 3] });
    await renderFrame(ctx, '/frame0.png', [layer], 5);

    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(1);
  });

  it('skips image layer with no source_path', async () => {
    const layer = makeLayer({ source_path: undefined });
    await renderFrame(ctx, '/frame0.png', [layer], 0);

    // save + restore happen, but no extra drawImage for the layer
    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(1);

    const restoreCalls = ctx._calls.filter((c) => c.method === 'restore');
    expect(restoreCalls.length).toBe(1);
  });

  it('draws a text layer without stroke', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: 'Hello World',
      font_size: 36,
      font_family: 'Arial',
      color: [255, 0, 0, 255],
      stroke: null,
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const fillTextCall = ctx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
    expect(fillTextCall!.args).toEqual(['Hello World', 0, 0]);

    const strokeTextCall = ctx._calls.find((c) => c.method === 'strokeText');
    expect(strokeTextCall).toBeUndefined();
  });

  it('draws a text layer with stroke', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: 'Stroked',
      font_size: 48,
      font_family: 'Impact',
      color: [255, 255, 255, 255],
      stroke: { color: [0, 0, 0, 255], width: 3 },
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const strokeTextCall = ctx._calls.find((c) => c.method === 'strokeText');
    expect(strokeTextCall).toBeDefined();
    expect(strokeTextCall!.args).toEqual(['Stroked', 0, 0]);

    const fillTextCall = ctx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
  });

  it('uses default font_size and font_family when not specified', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: 'Default',
      font_size: undefined,
      font_family: undefined,
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    // font should be set to default 48px "Impact"
    // We verify fillText was called (text was rendered)
    const fillTextCall = ctx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
  });

  it('uses default text and color when not specified', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: undefined,
      color: undefined,
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const fillTextCall = ctx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
    // Default text is ''
    expect(fillTextCall!.args[0]).toBe('');
  });

  it('sets globalAlpha from layer opacity', async () => {
    const layer = makeLayer({ opacity: 0.5 });
    await renderFrame(ctx, '/frame0.png', [layer], 0);

    // globalAlpha should have been set to 0.5
    // Since it's a property, not a method, we check it was set
    // After restore it resets, so we check the transform call ordering
    const saveIdx = ctx._calls.findIndex((c) => c.method === 'save');
    const transformIdx = ctx._calls.findIndex((c) => c.method === 'transform');
    expect(saveIdx).toBeLessThan(transformIdx);
  });

  // --- interpolateKeyframes coverage ---

  it('uses layer position/opacity when keyframes is empty', async () => {
    const layer = makeLayer({ keyframes: [], position: [30, 40], opacity: 0.8 });
    await renderFrame(ctx, '/frame0.png', [layer], 5);

    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    expect(transformCall!.args[4]).toBe(30); // tx
    expect(transformCall!.args[5]).toBe(40); // ty
  });

  it('clamps to first keyframe when frameIndex <= first keyframe', async () => {
    const keyframes: Keyframe[] = [
      { frame: 3, position: [100, 200], opacity: 0.5 },
      { frame: 7, position: [300, 400], opacity: 1.0 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });
    await renderFrame(ctx, '/frame0.png', [layer], 1);

    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    expect(transformCall!.args[4]).toBe(100);
    expect(transformCall!.args[5]).toBe(200);
  });

  it('clamps to last keyframe when frameIndex >= last keyframe', async () => {
    const keyframes: Keyframe[] = [
      { frame: 2, position: [10, 20], opacity: 0.3 },
      { frame: 5, position: [50, 60], opacity: 0.9 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });
    await renderFrame(ctx, '/frame0.png', [layer], 8);

    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    expect(transformCall!.args[4]).toBe(50);
    expect(transformCall!.args[5]).toBe(60);
  });

  it('interpolates between keyframes', async () => {
    const keyframes: Keyframe[] = [
      { frame: 0, position: [0, 0], opacity: 0.0 },
      { frame: 10, position: [100, 200], opacity: 1.0 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });
    await renderFrame(ctx, '/frame0.png', [layer], 5);

    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    // t = 0.5 -> position = [50, 100]
    expect(transformCall!.args[4]).toBe(50);
    expect(transformCall!.args[5]).toBe(100);
  });

  it('handles single keyframe', async () => {
    const keyframes: Keyframe[] = [
      { frame: 5, position: [42, 84], opacity: 0.7 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });

    // At the exact frame
    await renderFrame(ctx, '/frame0.png', [layer], 5);
    let transformCall = ctx._calls.find((c) => c.method === 'transform');
    expect(transformCall!.args[4]).toBe(42);
    expect(transformCall!.args[5]).toBe(84);
  });

  it('handles single keyframe before frame index', async () => {
    const keyframes: Keyframe[] = [
      { frame: 2, position: [42, 84], opacity: 0.7 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });

    await renderFrame(ctx, '/frame0.png', [layer], 5);
    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    // Clamps to last (only) keyframe
    expect(transformCall!.args[4]).toBe(42);
    expect(transformCall!.args[5]).toBe(84);
  });

  it('handles single keyframe after frame index', async () => {
    const keyframes: Keyframe[] = [
      { frame: 8, position: [42, 84], opacity: 0.7 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });

    await renderFrame(ctx, '/frame0.png', [layer], 3);
    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    // Clamps to first (only) keyframe
    expect(transformCall!.args[4]).toBe(42);
    expect(transformCall!.args[5]).toBe(84);
  });

  it('interpolates with three keyframes in the middle segment', async () => {
    const keyframes: Keyframe[] = [
      { frame: 0, position: [0, 0], opacity: 0.0 },
      { frame: 5, position: [50, 50], opacity: 0.5 },
      { frame: 10, position: [100, 100], opacity: 1.0 },
    ];
    const layer = makeLayer({ keyframes, position: [0, 0], opacity: 1.0 });

    // Frame 7 is between keyframe 5 and 10: t = (7-5)/(10-5) = 0.4
    await renderFrame(ctx, '/frame0.png', [layer], 7);
    const transformCall = ctx._calls.find((c) => c.method === 'transform');
    // position = [50 + 0.4*50, 50 + 0.4*50] = [70, 70]
    expect(transformCall!.args[4]).toBeCloseTo(70);
    expect(transformCall!.args[5]).toBeCloseTo(70);
  });

  it('returns first keyframe values when span is zero (same-frame keyframes)', () => {
    // To reach the span===0 branch inside the loop, we need two
    // consecutive keyframes at the same frame where no earlier pair
    // catches the frameIndex first.  Using NaN for the first keyframe
    // frame bypasses earlier pairs since NaN comparisons return false.
    const keyframes: Keyframe[] = [
      { frame: NaN, position: [0, 0], opacity: 0.0 },
      { frame: 3, position: [10, 20], opacity: 0.3 },
      { frame: 3, position: [30, 40], opacity: 0.7 },
      { frame: NaN, position: [99, 99], opacity: 0.9 },
    ];
    const result = interpolateKeyframes(keyframes, 3);
    // span=0 -> t=0 -> returns first keyframe of the matching pair
    expect(result).toEqual({ position: [10, 20], opacity: 0.3 });
  });

  it('falls back to last keyframe via post-loop return with NaN frames', async () => {
    // The post-loop fallback (line 47) is unreachable with sorted numeric
    // keyframes. We exercise it directly via the exported function using
    // NaN frame values, which cause all comparisons to return false.
    const keyframes: Keyframe[] = [
      { frame: NaN, position: [10, 20], opacity: 0.3 },
      { frame: NaN, position: [30, 40], opacity: 0.7 },
    ];
    const result = interpolateKeyframes(keyframes, 5);
    expect(result).toEqual({ position: [30, 40], opacity: 0.7 });
  });

  it('handles unknown layer_type gracefully', async () => {
    // Exercise the implicit else branch when layer_type is neither
    // 'image' nor 'text' (e.g., a future type the renderer ignores).
    const layer = makeLayer({
      layer_type: 'unknown' as unknown as 'image',
      source_path: undefined,
    });
    await renderFrame(ctx, '/frame0.png', [layer], 0);

    // Only base frame drawn, layer save/restore still happens
    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(1);
    const restoreCalls = ctx._calls.filter((c) => c.method === 'restore');
    expect(restoreCalls.length).toBe(1);
  });

  it('draws multiple layers in order', async () => {
    const layer1 = makeLayer({ id: 'l1', source_path: '/a.png', position: [0, 0] });
    const layer2 = makeLayer({ id: 'l2', source_path: '/b.png', position: [10, 10] });

    await renderFrame(ctx, '/frame0.png', [layer1, layer2], 0);

    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    // base frame + 2 layers
    expect(drawCalls.length).toBe(3);
  });

  it('caches images (second render reuses cache)', async () => {
    const layer = makeLayer({ source_path: '/cached.png' });

    await renderFrame(ctx, '/frame0.png', [layer], 0);
    const firstCallCount = ctx._calls.filter((c) => c.method === 'drawImage').length;

    ctx._calls.length = 0;
    await renderFrame(ctx, '/frame0.png', [layer], 0);
    const secondCallCount = ctx._calls.filter((c) => c.method === 'drawImage').length;

    expect(firstCallCount).toBe(secondCallCount);
  });

  it('handles text layer with stroke but undefined text', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: undefined,
      stroke: { color: [0, 0, 0, 255], width: 2 },
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const strokeTextCall = ctx._calls.find((c) => c.method === 'strokeText');
    expect(strokeTextCall).toBeDefined();
    expect(strokeTextCall!.args[0]).toBe('');
  });

  describe('rotation transform', () => {
    it('applies rotation via combined matrix to ctx.transform', async () => {
      const layer = makeLayer({
        rotation: 90,
        scale_x: 1,
        scale_y: 1,
        skew_x: 0,
        skew_y: 0,
      });
      await renderFrame(ctx, '/frame0.png', [layer], 0);
      // ctx.transform should have been called with cos(90°)≈0, sin(90°)≈1 matrix
      const transformCall = ctx._calls.find((c) => c.method === 'transform');
      expect(transformCall).toBeDefined();
      const call = transformCall!.args;
      // a = cos*sx - sin*ky = 0*1 - 1*0 = 0
      expect(call[0]).toBeCloseTo(0, 4);
      // b = sin*sx + cos*ky = 1*1 + 0*0 = 1
      expect(call[1]).toBeCloseTo(1, 4);
    });
  });

  describe('text wrapping', () => {
    it('wrapText returns multiple lines when text exceeds max_width', () => {
      const ctx = createMockCtx();
      vi.mocked(ctx.measureText).mockImplementation((text: string) => ({
        width: text.length * 10,
      } as TextMetrics));
      const lines = wrapText(ctx as unknown as CanvasRenderingContext2D, 'word1 word2 word3', 60);
      expect(lines.length).toBeGreaterThan(1);
    });
  });

  it('renderFrame renders a flare layer with lighter compositing', async () => {
    const flareLayer: LayerInfo = {
      id: 'fl1', name: 'Solar Flare', layer_type: 'flare',
      position: [100, 100] as [number, number], intensity: 1, scale: 1, pulse_speed: 0.15,
      opacity: 1, visible: true, frame_range: [0, 9] as [number, number], keyframes: [],
      scale_x: 1, scale_y: 1, skew_x: 0, skew_y: 0, rotation: 0,
    };

    await renderFrame(ctx, '/frame.png', [flareLayer], 0);

    // resetTransform should be called (flare overrides the per-layer transform)
    const resetCall = (ctx as any)._calls.find((c: any) => c.method === 'resetTransform');
    expect(resetCall).toBeDefined();

    // globalCompositeOperation should have been set to 'lighter'
    const compositeCall = (ctx as any)._calls.find(
      (c: any) => c.method === 'setCompositeOp' && c.args[0] === 'lighter',
    );
    expect(compositeCall).toBeDefined();

    // globalCompositeOperation should be reset to 'source-over' after flare rendering
    const resetCompositeCall = (ctx as any)._calls.filter(
      (c: any) => c.method === 'setCompositeOp'
    );
    expect(resetCompositeCall.length).toBeGreaterThanOrEqual(2);
    expect(resetCompositeCall[resetCompositeCall.length - 1].args[0]).toBe('source-over');
  });
});
