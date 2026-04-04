import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  // Prevent vite from obscuring Rust compiler errors
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true
  },
  test: {
    environment: 'jsdom',
    include: ['src/tests/**/*.test.ts'],
    globals: true,
  },
});
