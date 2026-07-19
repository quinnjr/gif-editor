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

  /**
   * Patch a layer in local state only, without a backend call (and therefore
   * without pushing a history entry). Used for live preview during gestures;
   * callers must persist the final value with `updateLayer` on gesture end.
   */
  updateLayerLocal(id: string, changes: Partial<LayerInfo>) {
    this.layers = this.layers.map((l) => (l.id === id ? { ...l, ...changes } : l));
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
    await this.syncFrameState();
  }

  async redo() {
    this.layers = await cmd.redo();
    await this.syncFrameState();
  }

  /**
   * Re-sync frame-related state after backend undo/redo, which restores
   * excluded frames as well as layers. When the excluded set changed, the
   * logical frame count shifts, so metadata is adjusted (the total source
   * frame count `visible + excluded` is invariant) and the framePaths cache
   * is invalidated — cached thumbnails are keyed by logical index, which is
   * no longer valid. Replacing the metadata object also triggers the
   * timeline strip to rebuild its thumbnails.
   */
  private async syncFrameState() {
    const excluded = await cmd.getExcludedFrames();
    const unchanged =
      excluded.length === this.excludedFrames.length &&
      excluded.every((f, i) => f === this.excludedFrames[i]);
    if (unchanged) return;

    if (this.metadata) {
      const totalFrames = this.metadata.frame_count + this.excludedFrames.length;
      this.metadata = { ...this.metadata, frame_count: totalFrames - excluded.length };
    }
    this.excludedFrames = excluded;
    this.framePaths = new Map();
  }

  async flipLayer(id: string, axis: 'horizontal' | 'vertical') {
    const updated = await cmd.flipLayer(id, axis);
    this.layers = this.layers.map((l) => (l.id === id ? updated : l));
  }

  async duplicateLayer(id: string) {
    const newLayer = await cmd.duplicateLayer(id);
    const idx = this.layers.findIndex((l) => l.id === id);
    const updated = [...this.layers];
    // Insert after the source layer; append if the source isn't in the
    // local list (backend accepted the id, so trust its result).
    updated.splice(idx === -1 ? updated.length : idx + 1, 0, newLayer);
    this.layers = updated;
    return newLayer;
  }

  async scaleAllLayers(scaleX: number, scaleY: number) {
    this.layers = await cmd.scaleAllLayers(scaleX, scaleY);
  }
}

export const project = new ProjectStore();
