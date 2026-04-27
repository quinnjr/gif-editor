import type { GifMetadata, LayerInfo } from '$lib/types';
import * as cmd from '$lib/commands';

class ProjectStore {
  metadata = $state<GifMetadata | null>(null);
  layers = $state<LayerInfo[]>([]);
  framePaths = $state<Map<number, string>>(new Map());
  excludedFrames = $state<number[]>([]);

  get isOpen() { return this.metadata !== null; }

  async open(path: string) {
    this.metadata = await cmd.openFile(path);
    this.layers = [];
    this.framePaths = new Map();
    this.excludedFrames = [];
  }

  async getFramePath(index: number): Promise<string> {
    if (this.framePaths.has(index)) return this.framePaths.get(index)!;
    const path = await cmd.getFrame(index);
    this.framePaths.set(index, path);
    return path;
  }

  async addImageLayer(path: string) {
    const [layer, newMeta] = await cmd.addImageLayer(path);
    this.layers = [...this.layers, layer];
    // When an animated GIF is added to a static image, the timeline expands.
    if (newMeta) {
      this.metadata = newMeta;
      this.framePaths = new Map();
    }
    return layer;
  }

  async addTextLayer(text: string) {
    const layer = await cmd.addTextLayer(text);
    this.layers = [...this.layers, layer];
    return layer;
  }

  async addFlareLayer(position?: [number, number]) {
    const layer = await cmd.addFlareLayer(position);
    this.layers = [...this.layers, layer];
    return layer;
  }

  async updateLayer(id: string, changes: Partial<LayerInfo>) {
    const updated = await cmd.updateLayer(id, changes);
    this.layers = this.layers.map((l) => (l.id === id ? updated : l));
    return updated;
  }

  async removeLayer(id: string) {
    await cmd.removeLayer(id);
    this.layers = this.layers.filter((l) => l.id !== id);
  }

  async reorderLayers(ids: string[]) {
    await cmd.reorderLayers(ids);
    const ordered = ids.map((id) => this.layers.find((l) => l.id === id)!);
    this.layers = ordered;
  }

  async refreshLayers() {
    this.layers = await cmd.getLayers();
  }

  async deleteFrames(logicalIndices: number[]) {
    this.metadata = await cmd.deleteFrames(logicalIndices);
    this.excludedFrames = await cmd.getExcludedFrames();
    this.layers = await cmd.getLayers();
    this.framePaths = new Map();
  }

  async restoreFrames(sourceIndices: number[]) {
    this.metadata = await cmd.restoreFrames(sourceIndices);
    this.excludedFrames = await cmd.getExcludedFrames();
    this.layers = await cmd.getLayers();
    this.framePaths = new Map();
  }

  async restoreAllFrames() {
    if (this.excludedFrames.length === 0) return;
    await this.restoreFrames([...this.excludedFrames]);
  }

  async undo() {
    this.layers = await cmd.undo();
  }

  async redo() {
    this.layers = await cmd.redo();
  }

  async flipLayer(id: string, axis: 'horizontal' | 'vertical') {
    const updated = await cmd.flipLayer(id, axis);
    this.layers = this.layers.map((l) => (l.id === id ? updated : l));
  }

  async duplicateLayer(id: string) {
    const newLayer = await cmd.duplicateLayer(id);
    const idx = this.layers.findIndex((l) => l.id === id);
    const updated = [...this.layers];
    updated.splice(idx + 1, 0, newLayer);
    this.layers = updated;
    return newLayer;
  }

  async scaleAllLayers(scaleX: number, scaleY: number) {
    this.layers = await cmd.scaleAllLayers(scaleX, scaleY);
  }
}

export const project = new ProjectStore();
