import { invoke } from '@tauri-apps/api/core';
import type { GifMetadata, LayerInfo, LayerUpdate, ExportSettings, Stroke } from './types';

export async function openFile(path: string): Promise<GifMetadata> {
  return invoke('open_file', { path });
}

export async function openGif(path: string): Promise<GifMetadata> {
  return invoke('open_gif', { path });
}

export async function getFrame(frameIndex: number): Promise<string> {
  return invoke('get_frame', { frameIndex });
}

export async function addImageLayer(path: string): Promise<[LayerInfo, GifMetadata | null]> {
  return invoke('add_image_layer', { path });
}

export async function addTextLayer(
  text: string,
  fontFamily?: string,
  fontSize?: number,
  color?: [number, number, number, number],
  stroke?: Stroke | null,
): Promise<LayerInfo> {
  return invoke('add_text_layer', { text, fontFamily, fontSize, color, stroke });
}

export async function updateLayer(id: string, changes: LayerUpdate): Promise<LayerInfo> {
  return invoke('update_layer', { id, changes });
}

export async function removeLayer(id: string): Promise<void> {
  return invoke('remove_layer', { id });
}

export async function reorderLayers(ids: string[]): Promise<void> {
  return invoke('reorder_layers', { ids });
}

export async function renderComposite(frameIndex: number): Promise<string> {
  return invoke('render_composite', { frameIndex });
}

export async function exportProject(settings: ExportSettings, outputPath: string): Promise<void> {
  return invoke('export_project', { settings, outputPath });
}

export async function getLayers(): Promise<LayerInfo[]> {
  return invoke('get_layers');
}

export async function getSystemFonts(): Promise<string[]> {
  return invoke('get_system_fonts');
}

export async function checkFfmpeg(): Promise<boolean> {
  return invoke('check_ffmpeg');
}

export async function deleteFrames(indices: number[]): Promise<GifMetadata> {
  return invoke('delete_frames', { indices });
}

export async function restoreFrames(sourceIndices: number[]): Promise<GifMetadata> {
  return invoke('restore_frames', { sourceIndices });
}

export async function getExcludedFrames(): Promise<number[]> {
  return invoke('get_excluded_frames');
}

export async function undo(): Promise<LayerInfo[]> {
  return invoke('undo');
}

export async function redo(): Promise<LayerInfo[]> {
  return invoke('redo');
}
