import type { GifMetadata, LayerInfo } from '$lib/types';
import * as cmd from '$lib/commands';

class ProjectStore {
  metadata = $state<GifMetadata | null>(null);
  layers = $state<LayerInfo[]>([]);
  framePaths = $state<Map<number, string>>(new Map());

  get isOpen() { return this.metadata !== null; }

  async open(path: string) {
    this.metadata = await cmd.openGif(path);
    this.layers = [];
    this.framePaths = new Map();
  }

  async getFramePath(index: number): Promise<string> {
    if (this.framePaths.has(index)) return this.framePaths.get(index)!;
    const path = await cmd.getFrame(index);
    this.framePaths.set(index, path);
    return path;
  }

  async addImageLayer(path: string) {
    const layer = await cmd.addImageLayer(path);
    this.layers = [...this.layers, layer];
    return layer;
  }

  async addTextLayer(text: string) {
    const layer = await cmd.addTextLayer(text);
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
}

export const project = new ProjectStore();
