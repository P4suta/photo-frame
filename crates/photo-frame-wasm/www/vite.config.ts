import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  base: './',
  server: { host: '0.0.0.0', port: 5173 },
  plugins: [solid()],
  build: {
    target: 'es2022',
    assetsInlineLimit: 0,
  },
});
