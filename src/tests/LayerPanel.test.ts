import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

describe('LayerPanel', () => {
  it('tauri mocks are available', () => {
    const { invoke } = await vi.importMock<typeof import('@tauri-apps/api/core')>('@tauri-apps/api/core');
    expect(invoke).toBeDefined();
    expect(typeof invoke).toBe('function');
  });
});
