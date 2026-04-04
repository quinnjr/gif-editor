class UiStore {
  selectedLayerId = $state<string | null>(null);
  currentFrame = $state(0);
  isPlaying = $state(false);
  playbackSpeed = $state(1.0);
  previewExport = $state(false);
  ffmpegAvailable = $state(false);

  selectLayer(id: string | null) { this.selectedLayerId = id; }
  setFrame(index: number) { this.currentFrame = index; }
  togglePlayback() { this.isPlaying = !this.isPlaying; }
  setPlaybackSpeed(speed: number) { this.playbackSpeed = speed; }
  togglePreviewExport() { this.previewExport = !this.previewExport; }
}

export const ui = new UiStore();
