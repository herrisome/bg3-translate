import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// @tauri-apps/cli 在 dev 时会注入以下 env：
//   TAURI_ENV_PLATFORM / TAURI_ENV_ARCH / TAURI_ENV_FAMILY
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  // Tauri 期望前端产物在 dist/
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite 的开发服务器配置。Tauri 通过此 host:port 加载前端。
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 让 Tauri 监听不到的目录不触发 HMR 重载
      ignored: ["**/src-tauri/**"],
    },
  },
}));
