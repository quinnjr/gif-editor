import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('$lib/commands', () => ({
  openFile: vi.fn(),
  getFrame: vi.fn(),
  addImageLayer: vi.fn(),
  addTextLayer: vi.fn(),
  updateLayer: vi.fn(),
  removeLayer: vi.fn(),
  reorderLayers: vi.fn(),
  getLayers: vi.fn(),
  deleteFrames: vi.fn(),
  restoreFrames: vi.fn(),
  getExcludedFrames: vi.fn(),
  undo: vi.fn(),
  redo: vi.fn(),
}));

import { project } from '$lib/stores/project.svelte';
import * as cmd from '$lib/commands';

const mockCmd = vi.mocked(cmd);

const fakeMeta = { frame_count: 10, width: 200, height: 200, delays: [100] };

function makeLayer(overrides: Record<string, unknown> = {}) {
  return {
    id: 'l1',
    name: 'Layer 1',
    layer_type: 'image' as const,
    position: [0, 0] as [number, number],
    scale_x: 1,
    scale_y: 1,
    skew_x: 0,
    skew_y: 0,
    opacity: 1,
    frame_range: [0, 9] as [number, number],
    visible: true,
    keyframes: [],
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  // Reset store state
  project.metadata = null;
  project.layers = [];
  project.framePaths = new Map();
  project.excludedFrames = [];
});

describe('ProjectStore', () => {
  describe('isOpen', () => {
    it('returns false when metadata is null', () => {
      expect(project.isOpen).toBe(false);
    });

    it('returns true when metadata is set', () => {
      project.metadata = fakeMeta;
      expect(project.isOpen).toBe(true);
    });
  });

  describe('open', () => {
    it('sets metadata and resets state', async () => {
      project.layers = [makeLayer()];
      project.framePaths = new Map([[0, '/tmp/frame0.png']]);
      project.excludedFrames = [1, 2];

      mockCmd.openFile.mockResolvedValue(fakeMeta);
      await project.open('/test.gif');

      expect(mockCmd.openFile).toHaveBeenCalledWith('/test.gif');
      expect(project.metadata).toEqual(fakeMeta);
      expect(project.layers).toEqual([]);
      expect(project.framePaths.size).toBe(0);
      expect(project.excludedFrames).toEqual([]);
    });
  });

  describe('getFramePath', () => {
    it('returns cached path when available', async () => {
      project.framePaths = new Map([[3, '/cached/frame3.png']]);
      const result = await project.getFramePath(3);
      expect(result).toBe('/cached/frame3.png');
      expect(mockCmd.getFrame).not.toHaveBeenCalled();
    });

    it('fetches and caches path when not cached', async () => {
      mockCmd.getFrame.mockResolvedValue('/tmp/frame5.png');
      const result = await project.getFramePath(5);
      expect(mockCmd.getFrame).toHaveBeenCalledWith(5);
      expect(result).toBe('/tmp/frame5.png');
      expect(project.framePaths.get(5)).toBe('/tmp/frame5.png');
    });
  });

  describe('addImageLayer', () => {
    it('adds layer without metadata update when newMeta is null', async () => {
      const layer = makeLayer();
      mockCmd.addImageLayer.mockResolvedValue([layer, null]);

      const result = await project.addImageLayer('/img.png');

      expect(mockCmd.addImageLayer).toHaveBeenCalledWith('/img.png');
      expect(result).toEqual(layer);
      expect(project.layers).toEqual([layer]);
      expect(project.metadata).toBeNull();
    });

    it('adds layer and updates metadata when newMeta is non-null', async () => {
      project.metadata = fakeMeta;
      project.framePaths = new Map([[0, '/old.png']]);

      const layer = makeLayer();
      const newMeta = { ...fakeMeta, frame_count: 20 };
      mockCmd.addImageLayer.mockResolvedValue([layer, newMeta]);

      const result = await project.addImageLayer('/anim.gif');

      expect(result).toEqual(layer);
      expect(project.layers).toEqual([layer]);
      expect(project.metadata).toEqual(newMeta);
      expect(project.framePaths.size).toBe(0);
    });

    it('appends to existing layers', async () => {
      const existing = makeLayer({ id: 'l0' });
      project.layers = [existing];
      const newLayer = makeLayer({ id: 'l1' });
      mockCmd.addImageLayer.mockResolvedValue([newLayer, null]);

      await project.addImageLayer('/img2.png');
      expect(project.layers).toEqual([existing, newLayer]);
    });
  });

  describe('addTextLayer', () => {
    it('adds text layer', async () => {
      const layer = makeLayer({ id: 'tl1', layer_type: 'text', text: 'Hello' });
      mockCmd.addTextLayer.mockResolvedValue(layer);

      const result = await project.addTextLayer('Hello');

      expect(mockCmd.addTextLayer).toHaveBeenCalledWith('Hello');
      expect(result).toEqual(layer);
      expect(project.layers).toEqual([layer]);
    });
  });

  describe('updateLayer', () => {
    it('updates the matching layer in the list', async () => {
      const l1 = makeLayer({ id: 'l1', name: 'Original' });
      const l2 = makeLayer({ id: 'l2', name: 'Other' });
      project.layers = [l1, l2];

      const updated = makeLayer({ id: 'l1', name: 'Updated' });
      mockCmd.updateLayer.mockResolvedValue(updated);

      const result = await project.updateLayer('l1', { name: 'Updated' });

      expect(mockCmd.updateLayer).toHaveBeenCalledWith('l1', { name: 'Updated' });
      expect(result).toEqual(updated);
      expect(project.layers[0]).toEqual(updated);
      expect(project.layers[1]).toEqual(l2);
    });

    it('does not change non-matching layers', async () => {
      const l1 = makeLayer({ id: 'l1' });
      project.layers = [l1];

      const updated = makeLayer({ id: 'l2', name: 'New' });
      mockCmd.updateLayer.mockResolvedValue(updated);

      await project.updateLayer('l2', { name: 'New' });
      // l1 stays unchanged because id doesn't match
      expect(project.layers[0]).toEqual(l1);
    });
  });

  describe('removeLayer', () => {
    it('removes the layer from the list', async () => {
      const l1 = makeLayer({ id: 'l1' });
      const l2 = makeLayer({ id: 'l2' });
      project.layers = [l1, l2];

      mockCmd.removeLayer.mockResolvedValue(undefined);
      await project.removeLayer('l1');

      expect(mockCmd.removeLayer).toHaveBeenCalledWith('l1');
      expect(project.layers).toEqual([l2]);
    });
  });

  describe('reorderLayers', () => {
    it('reorders layers according to ids', async () => {
      const l1 = makeLayer({ id: 'l1' });
      const l2 = makeLayer({ id: 'l2' });
      const l3 = makeLayer({ id: 'l3' });
      project.layers = [l1, l2, l3];

      mockCmd.reorderLayers.mockResolvedValue(undefined);
      await project.reorderLayers(['l3', 'l1', 'l2']);

      expect(mockCmd.reorderLayers).toHaveBeenCalledWith(['l3', 'l1', 'l2']);
      expect(project.layers).toEqual([l3, l1, l2]);
    });
  });

  describe('refreshLayers', () => {
    it('replaces layers with fresh data from backend', async () => {
      project.layers = [makeLayer({ id: 'old' })];
      const freshLayers = [makeLayer({ id: 'new1' }), makeLayer({ id: 'new2' })];
      mockCmd.getLayers.mockResolvedValue(freshLayers);

      await project.refreshLayers();

      expect(mockCmd.getLayers).toHaveBeenCalled();
      expect(project.layers).toEqual(freshLayers);
    });
  });

  describe('deleteFrames', () => {
    it('updates metadata, excluded frames, layers, and clears frame paths', async () => {
      project.framePaths = new Map([[0, '/old.png']]);
      const newMeta = { ...fakeMeta, frame_count: 8 };
      mockCmd.deleteFrames.mockResolvedValue(newMeta);
      mockCmd.getExcludedFrames.mockResolvedValue([0, 2]);
      mockCmd.getLayers.mockResolvedValue([]);

      await project.deleteFrames([0, 2]);

      expect(mockCmd.deleteFrames).toHaveBeenCalledWith([0, 2]);
      expect(project.metadata).toEqual(newMeta);
      expect(project.excludedFrames).toEqual([0, 2]);
      expect(project.layers).toEqual([]);
      expect(project.framePaths.size).toBe(0);
    });
  });

  describe('restoreFrames', () => {
    it('updates metadata, excluded frames, layers, and clears frame paths', async () => {
      project.framePaths = new Map([[0, '/old.png']]);
      const newMeta = { ...fakeMeta, frame_count: 10 };
      mockCmd.restoreFrames.mockResolvedValue(newMeta);
      mockCmd.getExcludedFrames.mockResolvedValue([]);
      mockCmd.getLayers.mockResolvedValue([makeLayer()]);

      await project.restoreFrames([1, 3]);

      expect(mockCmd.restoreFrames).toHaveBeenCalledWith([1, 3]);
      expect(project.metadata).toEqual(newMeta);
      expect(project.excludedFrames).toEqual([]);
      expect(project.layers).toEqual([makeLayer()]);
      expect(project.framePaths.size).toBe(0);
    });
  });

  describe('restoreAllFrames', () => {
    it('does nothing when excludedFrames is empty', async () => {
      project.excludedFrames = [];
      await project.restoreAllFrames();
      expect(mockCmd.restoreFrames).not.toHaveBeenCalled();
    });

    it('calls restoreFrames with all excluded frames', async () => {
      project.excludedFrames = [2, 5, 7];
      const newMeta = { ...fakeMeta, frame_count: 10 };
      mockCmd.restoreFrames.mockResolvedValue(newMeta);
      mockCmd.getExcludedFrames.mockResolvedValue([]);
      mockCmd.getLayers.mockResolvedValue([]);

      await project.restoreAllFrames();

      expect(mockCmd.restoreFrames).toHaveBeenCalledWith([2, 5, 7]);
    });
  });

  describe('undo/redo', () => {
    it('undo calls command and refreshes layers', async () => {
      const { undo } = await import('$lib/commands');
      vi.mocked(undo).mockResolvedValue([]);
      await project.undo();
      expect(undo).toHaveBeenCalledOnce();
      expect(project.layers).toEqual([]);
    });

    it('redo calls command and refreshes layers', async () => {
      const { redo } = await import('$lib/commands');
      vi.mocked(redo).mockResolvedValue([]);
      await project.redo();
      expect(redo).toHaveBeenCalledOnce();
    });
  });
});
