import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';

// The dev server proxies `/api` to the **void-control bridge**, not the
// daemon directly. The bridge terminates HTTP/TCP and re-dispatches to the
// daemon over whichever transport (AF_UNIX or TCP) it was configured with;
// browsers don't speak AF_UNIX, so this hop is mandatory once the daemon
// defaults to a Unix socket.
//
// Bridge listen address is configurable via `VITE_VOID_CONTROL_BRIDGE_TARGET`
// — read via `loadEnv` so values from `.env*` files reach the proxy config,
// not just shell-exported env. Default: 127.0.0.1:43210.
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const BRIDGE_TARGET =
    env.VITE_VOID_CONTROL_BRIDGE_TARGET ?? 'http://127.0.0.1:43210';

  return {
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
  };
});
