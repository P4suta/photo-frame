import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  base: './',
  server: {
    host: '0.0.0.0',
    port: 5173,
    // Phase F2 — COOP/COEP headers for SharedArrayBuffer. Without
    // these the page is not `crossOriginIsolated` and the WASM-
    // side `wasm-bindgen-rayon::init_thread_pool` no-ops. In dev
    // we set the headers directly; in production the bundled
    // `coi-serviceworker` (registered first thing in index.html)
    // synthesises the same effect on hosts that can't set headers
    // (GitHub Pages).
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },
  // Vite preview server (used by `just wasm-preview`) needs the
  // same headers — the SAB precondition is identical between dev
  // and preview surfaces.
  preview: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },
  plugins: [solid()],
  build: {
    target: 'es2022',
    assetsInlineLimit: 0,
  },
});
