import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri 自己管窗口，vite 不要抢着开浏览器；端口固定，devUrl 才对得上
  clearScreen: false,
  server: {
    port: 5273,
    strictPort: true,
  },
  build: {
    // Tauri 的 webview 是新版 Chromium/WebKit，不用降级到 ES5
    target: "esnext",
    sourcemap: true,
  },
});
