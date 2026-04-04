import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

describe('Timeline', () => {
  it('event listener mock returns unlisten function', async () => {
    const { listen } = await import('@tauri-apps/api/event');
    const unlisten = await listen('test-event', () => {});
    expect(typeof unlisten).toBe('function');
  });
});
