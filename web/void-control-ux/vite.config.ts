import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// The dev server proxies `/api` to the **void-control bridge**, not the
// daemon directly. The bridge terminates HTTP/TCP and re-dispatches to the
// daemon over whichever transport (AF_UNIX or TCP) it was configured with;
// browsers don't speak AF_UNIX, so this hop is mandatory once the daemon
// defaults to a Unix socket.
//
// Bridge listen address is configurable via `VOID_CONTROL_BRIDGE_LISTEN`
// in the bridge process; default is 127.0.0.1:43210.
const BRIDGE_TARGET =
  process.env.VITE_VOID_CONTROL_BRIDGE_TARGET ?? 'http://127.0.0.1:43210';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5174,
    proxy: {
      '/api': {
        target: BRIDGE_TARGET,
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, '')
      }
    }
  }
});
