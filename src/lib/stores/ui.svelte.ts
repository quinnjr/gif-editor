class UiStore {
  selectedLayerId = $state<string | null>(null);
  currentFrame = $state(0);
  isPlaying = $state(false);
  playbackSpeed = $state(1.0);
  previewExport = $state(false);
  ffmpegAvailable = $state(false);
  // Toast notification state. `toastId` increments on every showToast() call
  // so repeated identical messages still retrigger the Toast component.
  toastMessage = $state('');
  toastType = $state<'error' | 'success'>('error');
  toastId = $state(0);

  selectLayer(id: string | null) { this.selectedLayerId = id; }
  setFrame(index: number) { this.currentFrame = index; }
  togglePlayback() { this.isPlaying = !this.isPlaying; }
  setPlaying(playing: boolean) { this.isPlaying = playing; }
  setPlaybackSpeed(speed: number) { this.playbackSpeed = speed; }
  togglePreviewExport() { this.previewExport = !this.previewExport; }
  showToast(message: string, type: 'error' | 'success' = 'error') {
    this.toastMessage = message;
    this.toastType = type;
    this.toastId += 1;
  }
  showError(message: string) { this.showToast(message, 'error'); }
}

export const ui = new UiStore();
