class UiStore {
  selectedLayerId = $state<string | null>(null);
  currentFrame = $state(0);
  isPlaying = $state(false);
  playbackSpeed = $state(1.0);
  previewExport = $state(false);
  ffmpegAvailable = $state(false);
  // Infrastructure for future undo/redo state tracking
  // TODO: Wire up after each action to enable/disable undo/redo buttons dynamically
  canUndo = $state(false);
  canRedo = $state(false);

  selectLayer(id: string | null) { this.selectedLayerId = id; }
  setFrame(index: number) { this.currentFrame = index; }
  togglePlayback() { this.isPlaying = !this.isPlaying; }
  setPlaybackSpeed(speed: number) { this.playbackSpeed = speed; }
  togglePreviewExport() { this.previewExport = !this.previewExport; }
  setUndoState(canUndo: boolean, canRedo: boolean) {
    this.canUndo = canUndo;
    this.canRedo = canRedo;
  }
}

export const ui = new UiStore();
