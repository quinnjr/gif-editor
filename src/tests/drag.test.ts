import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createDragHandler } from '$lib/utils/drag';

describe('createDragHandler', () => {
  let onMove: ReturnType<typeof vi.fn>;
  let onEnd: ReturnType<typeof vi.fn>;
  let handler: ReturnType<typeof createDragHandler>;
  const addSpy = vi.spyOn(window, 'addEventListener');
  const removeSpy = vi.spyOn(window, 'removeEventListener');

  beforeEach(() => {
    onMove = vi.fn();
    onEnd = vi.fn();
    handler = createDragHandler(onMove, onEnd);
    addSpy.mockClear();
    removeSpy.mockClear();
  });

  afterEach(() => {
    // Clean up any lingering listeners by dispatching pointerup
    window.dispatchEvent(new PointerEvent('pointerup'));
  });

  function makePointerEvent(type: string, clientX: number, clientY: number, pointerId = 1) {
    const event = new PointerEvent(type, { clientX, clientY, pointerId, bubbles: true });
    return event;
  }

  it('returns an object with onPointerDown', () => {
    expect(handler).toHaveProperty('onPointerDown');
    expect(typeof handler.onPointerDown).toBe('function');
  });

  it('onPointerDown sets pointer capture and adds listeners', () => {
    const setPointerCapture = vi.fn();
    const target = document.createElement('div');
    target.setPointerCapture = setPointerCapture;

    const event = makePointerEvent('pointerdown', 100, 200, 42);
    Object.defineProperty(event, 'target', { value: target });

    handler.onPointerDown(event);

    expect(setPointerCapture).toHaveBeenCalledWith(42);
    expect(addSpy).toHaveBeenCalledWith('pointermove', expect.any(Function));
    expect(addSpy).toHaveBeenCalledWith('pointerup', expect.any(Function));
  });

  it('pointermove calls onMove with delta from start position', () => {
    const target = document.createElement('div');
    target.setPointerCapture = vi.fn();
    const downEvent = makePointerEvent('pointerdown', 100, 200);
    Object.defineProperty(downEvent, 'target', { value: target });
    handler.onPointerDown(downEvent);

    window.dispatchEvent(makePointerEvent('pointermove', 150, 230));

    expect(onMove).toHaveBeenCalledWith(50, 30);
  });

  it('pointerup removes listeners and calls onEnd', () => {
    const target = document.createElement('div');
    target.setPointerCapture = vi.fn();
    const downEvent = makePointerEvent('pointerdown', 100, 200);
    Object.defineProperty(downEvent, 'target', { value: target });
    handler.onPointerDown(downEvent);

    window.dispatchEvent(new PointerEvent('pointerup'));

    expect(removeSpy).toHaveBeenCalledWith('pointermove', expect.any(Function));
    expect(removeSpy).toHaveBeenCalledWith('pointerup', expect.any(Function));
    expect(onEnd).toHaveBeenCalledOnce();
  });

  it('onMove is not called after pointerup', () => {
    const target = document.createElement('div');
    target.setPointerCapture = vi.fn();
    const downEvent = makePointerEvent('pointerdown', 0, 0);
    Object.defineProperty(downEvent, 'target', { value: target });
    handler.onPointerDown(downEvent);

    // Release
    window.dispatchEvent(new PointerEvent('pointerup'));
    onMove.mockClear();

    // Further moves should not trigger callback
    window.dispatchEvent(makePointerEvent('pointermove', 50, 50));
    expect(onMove).not.toHaveBeenCalled();
  });

  it('handles multiple drag sequences', () => {
    const target = document.createElement('div');
    target.setPointerCapture = vi.fn();

    // First drag
    const down1 = makePointerEvent('pointerdown', 10, 20);
    Object.defineProperty(down1, 'target', { value: target });
    handler.onPointerDown(down1);
    window.dispatchEvent(makePointerEvent('pointermove', 30, 40));
    expect(onMove).toHaveBeenCalledWith(20, 20);
    window.dispatchEvent(new PointerEvent('pointerup'));
    expect(onEnd).toHaveBeenCalledTimes(1);

    onMove.mockClear();
    onEnd.mockClear();

    // Second drag
    const down2 = makePointerEvent('pointerdown', 50, 60);
    Object.defineProperty(down2, 'target', { value: target });
    handler.onPointerDown(down2);
    window.dispatchEvent(makePointerEvent('pointermove', 55, 65));
    expect(onMove).toHaveBeenCalledWith(5, 5);
    window.dispatchEvent(new PointerEvent('pointerup'));
    expect(onEnd).toHaveBeenCalledTimes(1);
  });
});
