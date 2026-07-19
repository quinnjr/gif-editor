import { describe, it, expect, beforeEach } from 'vitest';
import { ui } from '$lib/stores/ui.svelte';

describe('UiStore', () => {
  beforeEach(() => {
    // Reset to defaults
    ui.selectLayer(null);
    ui.setFrame(0);
    ui.isPlaying = false;
    ui.playbackSpeed = 1.0;
    ui.previewExport = false;
    ui.ffmpegAvailable = false;
  });

  describe('selectLayer', () => {
    it('sets selectedLayerId to a string', () => {
      ui.selectLayer('layer-1');
      expect(ui.selectedLayerId).toBe('layer-1');
    });

    it('sets selectedLayerId to null', () => {
      ui.selectLayer('layer-1');
      ui.selectLayer(null);
      expect(ui.selectedLayerId).toBeNull();
    });
  });

  describe('setFrame', () => {
    it('sets currentFrame', () => {
      ui.setFrame(5);
      expect(ui.currentFrame).toBe(5);
    });

    it('sets currentFrame to 0', () => {
      ui.setFrame(10);
      ui.setFrame(0);
      expect(ui.currentFrame).toBe(0);
    });
  });

  describe('togglePlayback', () => {
    it('toggles isPlaying from false to true', () => {
      expect(ui.isPlaying).toBe(false);
      ui.togglePlayback();
      expect(ui.isPlaying).toBe(true);
    });

    it('toggles isPlaying from true to false', () => {
      ui.isPlaying = true;
      ui.togglePlayback();
      expect(ui.isPlaying).toBe(false);
    });
  });

  describe('setPlaying', () => {
    it('starts playback', () => {
      ui.setPlaying(true);
      expect(ui.isPlaying).toBe(true);
    });

    it('stops playback', () => {
      ui.isPlaying = true;
      ui.setPlaying(false);
      expect(ui.isPlaying).toBe(false);
    });
  });

  describe('setPlaybackSpeed', () => {
    it('sets playbackSpeed', () => {
      ui.setPlaybackSpeed(2.0);
      expect(ui.playbackSpeed).toBe(2.0);
    });
  });

  describe('togglePreviewExport', () => {
    it('toggles previewExport from false to true', () => {
      expect(ui.previewExport).toBe(false);
      ui.togglePreviewExport();
      expect(ui.previewExport).toBe(true);
    });

    it('toggles previewExport from true to false', () => {
      ui.previewExport = true;
      ui.togglePreviewExport();
      expect(ui.previewExport).toBe(false);
    });
  });

  describe('showToast / showError', () => {
    it('sets message and type and increments toastId', () => {
      const before = ui.toastId;
      ui.showToast('hello', 'success');
      expect(ui.toastMessage).toBe('hello');
      expect(ui.toastType).toBe('success');
      expect(ui.toastId).toBe(before + 1);
    });

    it('showToast defaults to error type', () => {
      ui.showToast('oops');
      expect(ui.toastType).toBe('error');
    });

    it('increments toastId for repeated identical messages so the toast re-shows', () => {
      ui.showError('same message');
      const first = ui.toastId;
      ui.showError('same message');
      expect(ui.toastMessage).toBe('same message');
      expect(ui.toastType).toBe('error');
      expect(ui.toastId).toBe(first + 1);
    });
  });

  describe('ffmpegAvailable', () => {
    it('defaults to false', () => {
      expect(ui.ffmpegAvailable).toBe(false);
    });

    it('can be set to true', () => {
      ui.ffmpegAvailable = true;
      expect(ui.ffmpegAvailable).toBe(true);
    });
  });
});
