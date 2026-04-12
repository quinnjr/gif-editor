import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import {
  openFile,
  openGif,
  getFrame,
  addImageLayer,
  addTextLayer,
  addFlareLayer,
  updateLayer,
  removeLayer,
  reorderLayers,
  renderComposite,
  exportProject,
  getLayers,
  getSystemFonts,
  checkFfmpeg,
  deleteFrames,
  restoreFrames,
  getExcludedFrames,
} from '$lib/commands';

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockReset();
});

describe('commands', () => {
  it('openFile calls invoke with correct args', async () => {
    const meta = { frame_count: 10, width: 100, height: 100, delays: [100] };
    mockedInvoke.mockResolvedValue(meta);
    const result = await openFile('/test.gif');
    expect(mockedInvoke).toHaveBeenCalledWith('open_file', { path: '/test.gif' });
    expect(result).toEqual(meta);
  });

  it('openGif calls invoke with correct args', async () => {
    const meta = { frame_count: 5, width: 50, height: 50, delays: [50] };
    mockedInvoke.mockResolvedValue(meta);
    const result = await openGif('/test2.gif');
    expect(mockedInvoke).toHaveBeenCalledWith('open_gif', { path: '/test2.gif' });
    expect(result).toEqual(meta);
  });

  it('getFrame calls invoke with correct args', async () => {
    mockedInvoke.mockResolvedValue('/tmp/frame0.png');
    const result = await getFrame(0);
    expect(mockedInvoke).toHaveBeenCalledWith('get_frame', { frameIndex: 0 });
    expect(result).toBe('/tmp/frame0.png');
  });

  it('addImageLayer calls invoke with correct args', async () => {
    const layer = { id: 'l1' };
    const tuple: [unknown, null] = [layer, null];
    mockedInvoke.mockResolvedValue(tuple);
    const result = await addImageLayer('/img.png');
    expect(mockedInvoke).toHaveBeenCalledWith('add_image_layer', { path: '/img.png' });
    expect(result).toEqual(tuple);
  });

  it('addTextLayer calls invoke with correct args', async () => {
    const layer = { id: 'l2' };
    mockedInvoke.mockResolvedValue(layer);
    const result = await addTextLayer('Hello', 'Arial', 24, [255, 0, 0, 255], { color: [0, 0, 0, 255], width: 2 });
    expect(mockedInvoke).toHaveBeenCalledWith('add_text_layer', {
      text: 'Hello',
      fontFamily: 'Arial',
      fontSize: 24,
      color: [255, 0, 0, 255],
      stroke: { color: [0, 0, 0, 255], width: 2 },
    });
    expect(result).toEqual(layer);
  });

  it('addTextLayer works with optional params omitted', async () => {
    const layer = { id: 'l3' };
    mockedInvoke.mockResolvedValue(layer);
    const result = await addTextLayer('Hi');
    expect(mockedInvoke).toHaveBeenCalledWith('add_text_layer', {
      text: 'Hi',
      fontFamily: undefined,
      fontSize: undefined,
      color: undefined,
      stroke: undefined,
    });
    expect(result).toEqual(layer);
  });

  it('updateLayer calls invoke with correct args', async () => {
    const updated = { id: 'l1', name: 'renamed' };
    mockedInvoke.mockResolvedValue(updated);
    const result = await updateLayer('l1', { name: 'renamed' });
    expect(mockedInvoke).toHaveBeenCalledWith('update_layer', { id: 'l1', changes: { name: 'renamed' } });
    expect(result).toEqual(updated);
  });

  it('removeLayer calls invoke with correct args', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    await removeLayer('l1');
    expect(mockedInvoke).toHaveBeenCalledWith('remove_layer', { id: 'l1' });
  });

  it('reorderLayers calls invoke with correct args', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    await reorderLayers(['l2', 'l1']);
    expect(mockedInvoke).toHaveBeenCalledWith('reorder_layers', { ids: ['l2', 'l1'] });
  });

  it('renderComposite calls invoke with correct args', async () => {
    mockedInvoke.mockResolvedValue('data:image/png;base64,...');
    const result = await renderComposite(3);
    expect(mockedInvoke).toHaveBeenCalledWith('render_composite', { frameIndex: 3 });
    expect(result).toBe('data:image/png;base64,...');
  });

  it('exportProject calls invoke with correct args', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    const settings = { format: 'Gif' as const, quality: 80, resize: null };
    await exportProject(settings, '/out.gif');
    expect(mockedInvoke).toHaveBeenCalledWith('export_project', { settings, outputPath: '/out.gif' });
  });

  it('getLayers calls invoke with no extra args', async () => {
    mockedInvoke.mockResolvedValue([]);
    const result = await getLayers();
    expect(mockedInvoke).toHaveBeenCalledWith('get_layers');
    expect(result).toEqual([]);
  });

  it('getSystemFonts calls invoke with no extra args', async () => {
    mockedInvoke.mockResolvedValue(['Arial', 'Helvetica']);
    const result = await getSystemFonts();
    expect(mockedInvoke).toHaveBeenCalledWith('get_system_fonts');
    expect(result).toEqual(['Arial', 'Helvetica']);
  });

  it('checkFfmpeg calls invoke with no extra args', async () => {
    mockedInvoke.mockResolvedValue(true);
    const result = await checkFfmpeg();
    expect(mockedInvoke).toHaveBeenCalledWith('check_ffmpeg');
    expect(result).toBe(true);
  });

  it('deleteFrames calls invoke with correct args', async () => {
    const meta = { frame_count: 8, width: 100, height: 100, delays: [100] };
    mockedInvoke.mockResolvedValue(meta);
    const result = await deleteFrames([0, 2]);
    expect(mockedInvoke).toHaveBeenCalledWith('delete_frames', { indices: [0, 2] });
    expect(result).toEqual(meta);
  });

  it('restoreFrames calls invoke with correct args', async () => {
    const meta = { frame_count: 10, width: 100, height: 100, delays: [100] };
    mockedInvoke.mockResolvedValue(meta);
    const result = await restoreFrames([1, 3]);
    expect(mockedInvoke).toHaveBeenCalledWith('restore_frames', { sourceIndices: [1, 3] });
    expect(result).toEqual(meta);
  });

  it('getExcludedFrames calls invoke with no extra args', async () => {
    mockedInvoke.mockResolvedValue([2, 4]);
    const result = await getExcludedFrames();
    expect(mockedInvoke).toHaveBeenCalledWith('get_excluded_frames');
    expect(result).toEqual([2, 4]);
  });

  it('addFlareLayer calls invoke with null position when omitted', async () => {
    const layer = { id: 'f1', layer_type: 'flare', intensity: 1, scale: 1, pulse_speed: 0.15 };
    mockedInvoke.mockResolvedValue(layer);
    const result = await addFlareLayer();
    expect(mockedInvoke).toHaveBeenCalledWith('add_flare_layer', { position: null });
    expect(result).toEqual(layer);
  });

  it('addFlareLayer passes position when provided', async () => {
    const layer = { id: 'f2', layer_type: 'flare' };
    mockedInvoke.mockResolvedValue(layer);
    await addFlareLayer([100, 200]);
    expect(mockedInvoke).toHaveBeenCalledWith('add_flare_layer', { position: [100, 200] });
  });
});
