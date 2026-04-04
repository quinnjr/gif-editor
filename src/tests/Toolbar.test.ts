import { describe, it, expect, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => path),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

describe('Toolbar', () => {
  it('dialog mock open function is available', () => {
    const { open } = vi.mocked(require('@tauri-apps/plugin-dialog'));
    expect(open).toBeDefined();
    expect(typeof open).toBe('function');
  });
});
