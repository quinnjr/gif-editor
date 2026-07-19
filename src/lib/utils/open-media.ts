import { open } from '@tauri-apps/plugin-dialog';
import { project } from '$lib/stores/project.svelte';
import { ui } from '$lib/stores/ui.svelte';

/** Single source of the formats the app can open (mirrors open_file). */
export const MEDIA_EXTENSIONS = ['gif', 'mp4', 'webm', 'png', 'jpg', 'jpeg', 'webp'];

/**
 * Pick a media file and open it as the project, landing on frame 0.
 * Every failure (including a dialog fault) surfaces via the toast; a
 * cancelled dialog is a no-op.
 */
export async function openMediaFile(): Promise<void> {
  try {
    const path = await open({
      filters: [{ name: 'Supported Media', extensions: MEDIA_EXTENSIONS }],
    });
    if (!path) return;
    await project.open(path);
    ui.setFrame(0);
  } catch (e) {
    ui.showError(`Failed to open file: ${e}`);
  }
}
