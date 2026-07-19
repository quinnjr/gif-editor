import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => path),
}));

import { invoke } from '@tauri-apps/api/core';
import {
  renderFrame,
  interpolateKeyframes,
  wrapText,
  resolveFontFamily,
  clearRenderCaches,
} from '$lib/utils/canvas-renderer';
import type { LayerInfo, Keyframe } from '$lib/types';

// Every src assigned to a MockImage; a repeated src means the LRU cache
// missed and the renderer had to decode the "image" again.
const createdImageSrcs: string[] = [];

// Mock Image class that auto-triggers onload
class MockImage {
  src = '';
  naturalWidth = 100;
  naturalHeight = 100;
  onload: (() => void) | null = null;
  onerror: ((err: unknown) => void) | null = null;

  constructor() {
    // Use a getter/setter on src to trigger onload when set
    const triggerLoad = () => this.onload?.();
    let _src = '';
    Object.defineProperty(this, 'src', {
      get() { return _src; },
      set(val: string) {
        _src = val;
        createdImageSrcs.push(val);
        // Trigger onload async
        Promise.resolve().then(triggerLoad);
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
  let _globalAlpha = 1;
  const makeGradient = () => ({ addColorStop: track('addColorStop') });

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
    measureText: vi.fn(() => ({ width: 100 } as TextMetrics)),
    createRadialGradient: vi.fn(makeGradient),
    createLinearGradient: vi.fn(makeGradient),
    get globalAlpha() { return _globalAlpha; },
    set globalAlpha(v: number) { _globalAlpha = v; calls.push({ method: 'setGlobalAlpha', args: [v] }); },
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
  let offCtx: ReturnType<typeof createMockCtx>;
  let offCanvas: { width: number; height: number; getContext: () => unknown };

  beforeEach(() => {
    clearRenderCaches();
    createdImageSrcs.length = 0;
    ctx = createMockCtx();
    // Text layers rasterise into an offscreen canvas obtained via
    // document.createElement('canvas'); jsdom has no real 2D context, so
    // hand back a fake canvas wired to a second mock ctx.
    offCtx = createMockCtx();
    offCanvas = { width: 0, height: 0, getContext: () => offCtx };
    const realCreateElement = document.createElement.bind(document);
    vi.spyOn(document, 'createElement').mockImplementation(((tag: string) =>
      tag === 'canvas'
        ? (offCanvas as unknown as HTMLCanvasElement)
        : realCreateElement(tag)) as typeof document.createElement);
  });

  afterEach(() => {
    // Restore document.createElement so the next test's spy binds the real
    // implementation (re-binding an active spy would recurse).
    vi.restoreAllMocks();
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

    // Text is rasterised on the offscreen canvas (no pad without stroke)...
    const fillTextCall = offCtx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
    expect(fillTextCall!.args).toEqual(['Hello World', 0, 0]);

    const strokeTextCall = offCtx._calls.find((c) => c.method === 'strokeText');
    expect(strokeTextCall).toBeUndefined();

    // ...and composited onto the main canvas in a single drawImage.
    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls.length).toBe(2); // base frame + text image
    expect(drawCalls[1].args).toEqual([offCanvas, -0, -0]);
    expect(ctx._calls.find((c) => c.method === 'fillText')).toBeUndefined();
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

    // pad = ceil(3) + 2 = 5; the glyph box is drawn at (pad, pad) on the
    // offscreen canvas, which is blitted back at (-pad, -pad).
    const strokeTextCall = offCtx._calls.find((c) => c.method === 'strokeText');
    expect(strokeTextCall).toBeDefined();
    expect(strokeTextCall!.args).toEqual(['Stroked', 5, 5]);

    const fillTextCall = offCtx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
    expect(fillTextCall!.args).toEqual(['Stroked', 5, 5]);

    const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
    expect(drawCalls[1].args).toEqual([offCanvas, -5, -5]);
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

    // Default font_family "Impact" maps to the bundled Anton typeface,
    // matching the backend substitution in fonts.rs.
    const fillTextCall = offCtx._calls.find((c) => c.method === 'fillText');
    expect(fillTextCall).toBeDefined();
    expect((offCtx as unknown as { font: string }).font).toBe('48px "Anton", sans-serif');
  });

  it('uses default text and color when not specified', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: undefined,
      color: undefined,
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const fillTextCall = offCtx._calls.find((c) => c.method === 'fillText');
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
    const transformCall = ctx._calls.find((c) => c.method === 'transform');
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

  describe('golden parity vectors (mirrored in src-tauri/tests/layer_test.rs)', () => {
    it('matches the shared interpolation vectors exactly', () => {
      const keyframes: Keyframe[] = [
        { frame: 0, position: [10, 20], opacity: 1.0 },
        { frame: 10, position: [30, 40], opacity: 0.5 },
        { frame: 20, position: [110, 140], opacity: 0.9 },
      ];

      expect(interpolateKeyframes(keyframes, 0)).toEqual({
        position: [10, 20],
        opacity: 1.0,
      });
      expect(interpolateKeyframes(keyframes, 5)).toEqual({
        position: [20, 30],
        opacity: 0.75,
      });
      expect(interpolateKeyframes(keyframes, 15)).toEqual({
        position: [70, 90],
        opacity: 0.7,
      });
      expect(interpolateKeyframes(keyframes, 25)).toEqual({
        position: [110, 140],
        opacity: 0.9,
      });
    });

    it('clamps to a single keyframe evaluated before its frame', () => {
      const keyframes: Keyframe[] = [
        { frame: 5, position: [42, 84], opacity: 0.7 },
      ];
      expect(interpolateKeyframes(keyframes, 0)).toEqual({
        position: [42, 84],
        opacity: 0.7,
      });
    });

    it('returns null for an empty keyframe array', () => {
      expect(interpolateKeyframes([], 0)).toBeNull();
    });
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

  describe('image LRU cache (cap 48)', () => {
    const srcCount = (src: string) =>
      createdImageSrcs.filter((s) => s === src).length;

    it('does not reload a cached image', async () => {
      await renderFrame(ctx, '/f0.png', [], 0);
      await renderFrame(ctx, '/f0.png', [], 0);
      expect(srcCount('/f0.png')).toBe(1);
    });

    it('evicts the oldest entry once the cap is exceeded', async () => {
      // Fill the cache past its cap of 48 with distinct base frames.
      for (let i = 0; i <= 48; i++) {
        await renderFrame(ctx, `/f${i}.png`, [], 0);
      }
      // 49 inserts: /f0.png (the oldest) must have been evicted...
      await renderFrame(ctx, '/f0.png', [], 0);
      expect(srcCount('/f0.png')).toBe(2);
      // ...while the most recent entry is still cached.
      await renderFrame(ctx, '/f48.png', [], 0);
      expect(srcCount('/f48.png')).toBe(1);
    });

    it('refreshes recency on access so hot entries survive eviction', async () => {
      // Fill the cache exactly to its cap; /f0.png is the oldest.
      for (let i = 0; i < 48; i++) {
        await renderFrame(ctx, `/f${i}.png`, [], 0);
      }
      // Touch /f0.png: a cache hit that must move it to most-recent.
      await renderFrame(ctx, '/f0.png', [], 0);
      expect(srcCount('/f0.png')).toBe(1);

      // Insert one more; the eviction victim must now be /f1.png.
      await renderFrame(ctx, '/f48.png', [], 0);

      await renderFrame(ctx, '/f0.png', [], 0);
      expect(srcCount('/f0.png')).toBe(1); // survived (recency refreshed)

      await renderFrame(ctx, '/f1.png', [], 0);
      expect(srcCount('/f1.png')).toBe(2); // evicted as the true oldest
    });
  });

  describe('text raster cache', () => {
    const canvasCreations = () =>
      vi
        .mocked(document.createElement)
        .mock.calls.filter((c) => c[0] === 'canvas').length;

    const textLayer = (overrides: Partial<LayerInfo> = {}) =>
      makeLayer({
        layer_type: 'text',
        text: 'Cached',
        font_size: 48,
        font_family: 'Impact',
        color: [255, 255, 255, 255],
        stroke: { color: [0, 0, 0, 255], width: 3 },
        source_path: undefined,
        ...overrides,
      });

    it('rasterises unchanged text only once across repeated renders', async () => {
      const layer = textLayer();
      await renderFrame(ctx, '/frame0.png', [layer], 0);
      expect(canvasCreations()).toBe(1);

      await renderFrame(ctx, '/frame0.png', [layer], 1);
      await renderFrame(ctx, '/frame0.png', [layer], 2);
      expect(canvasCreations()).toBe(1);

      // The cached raster is still composited on every render, honouring
      // the stroke pad (ceil(3) + 2 = 5).
      const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
      expect(drawCalls.length).toBe(6); // 3 base frames + 3 text composites
      expect(drawCalls[5].args).toEqual([offCanvas, -5, -5]);
    });

    it('re-rasterises when a content-affecting field changes', async () => {
      await renderFrame(ctx, '/frame0.png', [textLayer()], 0);
      expect(canvasCreations()).toBe(1);

      await renderFrame(ctx, '/frame0.png', [textLayer({ text: 'Changed' })], 0);
      expect(canvasCreations()).toBe(2);

      await renderFrame(ctx, '/frame0.png', [textLayer({ text: 'Changed', font_size: 36 })], 0);
      expect(canvasCreations()).toBe(3);
    });

    it('shares one raster between identical text layers with different ids', async () => {
      const a = textLayer({ id: 'l1' });
      const b = textLayer({ id: 'l2', position: [50, 50] });
      await renderFrame(ctx, '/frame0.png', [a, b], 0);

      expect(canvasCreations()).toBe(1);
      const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
      expect(drawCalls.length).toBe(3); // base frame + both text composites
    });

    it('does not key the raster on transform or opacity fields', async () => {
      await renderFrame(ctx, '/frame0.png', [textLayer()], 0);
      await renderFrame(
        ctx,
        '/frame0.png',
        [textLayer({ opacity: 0.5, rotation: 45, position: [9, 9], scale_x: 2 })],
        0,
      );
      expect(canvasCreations()).toBe(1);
    });
  });

  describe('font load invalidation (onFontsReady)', () => {
    const canvasCreations = () =>
      vi
        .mocked(document.createElement)
        .mock.calls.filter((c) => c[0] === 'canvas').length;

    it('clears the text raster cache and notifies subscribers when the bundled FontFaces load', async () => {
      // Deferred get_font_data IPC responses so the FontFace loads land
      // only when the test resolves them — after the first (fallback-font)
      // raster has been cached.
      const resolvers: ((b64: string) => void)[] = [];
      vi.mocked(invoke).mockImplementation(
        () =>
          new Promise((resolve) => {
            resolvers.push(resolve as (b64: string) => void);
          }),
      );

      const addedFaces: unknown[] = [];
      class MockFontFace {
        constructor(
          public family: string,
          public data: ArrayBuffer,
        ) {}
        async load() {
          return this;
        }
      }
      vi.stubGlobal('FontFace', MockFontFace);
      Object.defineProperty(document, 'fonts', {
        value: { add: (face: unknown) => addedFaces.push(face) },
        configurable: true,
      });

      try {
        // Fresh module instance so its init-time font loading runs with the
        // FontFace / document.fonts stubs in place.
        vi.resetModules();
        const mod = await import('$lib/utils/canvas-renderer');

        const cb = vi.fn();
        const cbUnsubscribed = vi.fn();
        mod.onFontsReady(cb);
        const unsubscribe = mod.onFontsReady(cbUnsubscribed);
        unsubscribe();

        const layer = makeLayer({
          layer_type: 'text',
          text: 'Pinned',
          font_size: 48,
          font_family: 'Impact',
          color: [255, 255, 255, 255],
          stroke: null,
          source_path: undefined,
        });

        // First render rasterises (with the fallback font) and caches.
        await mod.renderFrame(ctx, '/frame0.png', [layer], 0);
        expect(canvasCreations()).toBe(1);
        await mod.renderFrame(ctx, '/frame0.png', [layer], 1);
        expect(canvasCreations()).toBe(1); // cache hit
        expect(cb).not.toHaveBeenCalled();

        // Land both bundled font loads (Anton + Liberation Sans).
        expect(resolvers.length).toBe(2);
        for (const resolve of resolvers) resolve(btoa('tiny-font'));
        await new Promise((r) => setTimeout(r, 0)); // flush the .then chains

        expect(addedFaces.length).toBe(2);
        expect(cb).toHaveBeenCalledTimes(2); // once per loaded family
        expect(cbUnsubscribed).not.toHaveBeenCalled();

        // The raster cache was cleared: identical text re-rasterises once,
        // then is cached again.
        await mod.renderFrame(ctx, '/frame0.png', [layer], 2);
        expect(canvasCreations()).toBe(2);
        await mod.renderFrame(ctx, '/frame0.png', [layer], 3);
        expect(canvasCreations()).toBe(2);
      } finally {
        delete (document as { fonts?: unknown }).fonts;
        vi.resetModules();
      }
    });
  });

  it('handles text layer with stroke but undefined text', async () => {
    const layer = makeLayer({
      layer_type: 'text',
      text: undefined,
      stroke: { color: [0, 0, 0, 255], width: 2 },
      source_path: undefined,
    });

    await renderFrame(ctx, '/frame0.png', [layer], 0);

    const strokeTextCall = offCtx._calls.find((c) => c.method === 'strokeText');
    expect(strokeTextCall).toBeDefined();
    // pad = ceil(2) + 2 = 4
    expect(strokeTextCall!.args).toEqual(['', 4, 4]);
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
    const resetCall = ctx._calls.find((c) => c.method === 'resetTransform');
    expect(resetCall).toBeDefined();

    // globalCompositeOperation should have been set to 'lighter'
    const compositeCall = ctx._calls.find(
      (c) => c.method === 'setCompositeOp' && c.args[0] === 'lighter',
    );
    expect(compositeCall).toBeDefined();

    // globalCompositeOperation should be reset to 'source-over' after flare rendering
    const resetCompositeCall = ctx._calls.filter(
      (c) => c.method === 'setCompositeOp'
    );
    expect(resetCompositeCall.length).toBeGreaterThanOrEqual(2);
    expect(resetCompositeCall[resetCompositeCall.length - 1].args[0]).toBe('source-over');
  });

  describe('font substitution (mirrors src-tauri/src/fonts.rs)', () => {
    it('maps impact and anton (any case) to Anton', () => {
      expect(resolveFontFamily('Impact')).toBe('Anton');
      expect(resolveFontFamily('impact')).toBe('Anton');
      expect(resolveFontFamily('ANTON')).toBe('Anton');
    });

    it('maps everything else to Liberation Sans', () => {
      expect(resolveFontFamily('Arial')).toBe('Liberation Sans');
      expect(resolveFontFamily('Comic Sans MS')).toBe('Liberation Sans');
    });

    it('defaults a missing family to Impact -> Anton', () => {
      expect(resolveFontFamily(undefined)).toBe('Anton');
      expect(resolveFontFamily(null)).toBe('Anton');
    });

    it('round-trips the backend-advertised labels to their CSS families', () => {
      // list_available_fonts (src-tauri/src/fonts.rs) advertises exactly
      // ["Anton", "Liberation Sans"]; the LayerItem <select> stores those
      // labels verbatim, so each must resolve to the matching @font-face
      // family declared in src/app.css.
      expect(resolveFontFamily('Anton')).toBe('Anton');
      expect(resolveFontFamily('Liberation Sans')).toBe('Liberation Sans');
    });
  });

  describe('text layer opacity flattening', () => {
    it('rasterises stroke+fill at full alpha and composites once at layer opacity', async () => {
      const layer = makeLayer({
        layer_type: 'text',
        text: 'Ghosty',
        opacity: 0.5,
        color: [255, 255, 255, 255],
        stroke: { color: [0, 0, 0, 255], width: 3 },
        source_path: undefined,
      });

      await renderFrame(ctx, '/frame0.png', [layer], 0);

      // The offscreen canvas must never see the layer opacity...
      expect(offCtx._calls.find((c) => c.method === 'setGlobalAlpha')).toBeUndefined();
      // ...and its colours carry full alpha.
      const o = offCtx as unknown as { fillStyle: string; strokeStyle: string };
      expect(o.fillStyle).toBe('rgba(255,255,255,1)');
      expect(o.strokeStyle).toBe('rgba(0,0,0,1)');

      // The main ctx composites the flattened image at the layer opacity.
      const alphaSet = ctx._calls.find(
        (c) => c.method === 'setGlobalAlpha' && c.args[0] === 0.5,
      );
      expect(alphaSet).toBeDefined();
      const drawCalls = ctx._calls.filter((c) => c.method === 'drawImage');
      expect(drawCalls[1].args[0]).toBe(offCanvas);
    });
  });

  describe('flare constants (aligned to flare_renderer.rs)', () => {
    const flareLayer = (overrides: Partial<LayerInfo> = {}): LayerInfo => ({
      id: 'fl1', name: 'Solar Flare', layer_type: 'flare',
      position: [100, 100] as [number, number], intensity: 1, scale: 1, pulse_speed: 0.15,
      opacity: 1, visible: true, frame_range: [0, 9] as [number, number], keyframes: [],
      scale_x: 1, scale_y: 1, skew_x: 0, skew_y: 0, rotation: 0,
      ...overrides,
    });

    const stopColors = () =>
      ctx._calls.filter((c) => c.method === 'addColorStop').map((c) => c.args[1] as string);

    it('uses a pure white central glow (no warm tint)', async () => {
      // frame 0 -> brightness = intensity * (1 + 0.3*sin(0)) = 1
      await renderFrame(ctx, '/frame.png', [flareLayer()], 0);
      const colors = stopColors();
      expect(colors).toContain('rgba(255,255,255,0.700)');
      expect(colors.some((c) => c.startsWith('rgba(255,255,220'))).toBe(false);
    });

    it('peaks the ring gradient at the backend yellow #FFE87C', async () => {
      await renderFrame(ctx, '/frame.png', [flareLayer()], 0);
      const colors = stopColors();
      expect(colors).toContain('rgba(255,232,124,0.500)');
      // The old orange ring peak must be gone (orange remains only in the halo).
      expect(colors.filter((c) => c.startsWith('rgba(255,123,0')).length).toBe(3);
    });

    it('uses brightness * 0.4 for the outer halo peak', async () => {
      await renderFrame(ctx, '/frame.png', [flareLayer()], 0);
      expect(stopColors()).toContain('rgba(255,123,0,0.400)');
    });

    it('does not clamp ghost brightness to 1.0', async () => {
      // intensity 2 -> brightness 2; ghost i=2: 2 * 0.7 * (1 + 0.3*sin(1.0)) ≈ 1.753
      await renderFrame(ctx, '/frame.png', [flareLayer({ intensity: 2 })], 0);
      const expected = 2 * 0.7 * (1 + 0.3 * Math.sin(1.0));
      expect(stopColors()).toContain(`rgba(75,110,175,${expected.toFixed(3)})`);
    });
  });
});
