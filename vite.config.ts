import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;
const appVersion = process.env.npm_package_version ?? "0.0.0";

function includesAny(value: string, patterns: string[]) {
  return patterns.some((pattern) => value.includes(pattern));
}

function manualChunks(id: string) {
  const normalized = id.replace(/\\/g, "/");

  if (!normalized.includes("/node_modules/")) {
    return undefined;
  }

  if (
    normalized.includes("/node_modules/react-markdown/")
    || normalized.includes("/node_modules/remark-gfm/")
    || normalized.includes("/node_modules/prismjs/")
  ) {
    return "markdown-rendering";
  }

  if (
    normalized.includes("/node_modules/codemirror/")
    || normalized.includes("/node_modules/@codemirror/")
    || normalized.includes("/node_modules/@lezer/")
  ) {
    return "codemirror";
  }

  if (normalized.includes("/node_modules/cmdk/")) {
    return "cmdk";
  }

  if (normalized.includes("/node_modules/@tanstack/react-query/")) {
    return "tanstack-query";
  }

  if (normalized.includes("/node_modules/zustand/")) {
    return "zustand";
  }

  if (normalized.includes("/node_modules/lucide-react/")) {
    return "lucide";
  }

  if (normalized.includes("/node_modules/@mdxeditor/gurx/")) {
    return "mdxeditor-gurx";
  }

  if (
    includesAny(normalized, [
      "/node_modules/@radix-ui/",
      "/node_modules/react-hook-form/",
      "/node_modules/downshift/",
      "/node_modules/classnames/",
    ])
  ) {
    return "mdxeditor-ui";
  }

  if (
    normalized.includes("/node_modules/@lexical/")
    || normalized.includes("/node_modules/lexical/")
  ) {
    return "mdxeditor-lexical";
  }

  if (normalized.includes("/node_modules/@tauri-apps/")) {
    return "tauri";
  }

  if (
    normalized.includes("/node_modules/react/")
    || normalized.includes("/node_modules/react-dom/")
    || normalized.includes("/node_modules/react-router-dom/")
    || normalized.includes("/node_modules/scheduler/")
  ) {
    return "react-vendor";
  }

  return undefined;
}

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  // Tauri WebView 是 Chromium/Edge WebView2 —— 直接编到现代 ES，避免无谓 polyfill / 降级。
  esbuild: {
    legalComments: "none",
    drop: ["debugger"],
    pure: ["console.debug", "console.trace"],
  },
  build: {
    target: "es2022",
    cssTarget: "chrome105",
    minify: "esbuild",
    sourcemap: false,
    cssCodeSplit: true,
    reportCompressedSize: false,
    chunkSizeWarningLimit: 1024,
    assetsInlineLimit: 4096,
    // Vite 默认对所有动态 import 的依赖链都会注入 <link rel="modulepreload">，
    // 这会让 codemirror（1.66MB）、mdxeditor 等纯按需 chunk 在冷启动就被下载，
    // 拖累首屏可交互时间。这里过滤掉重型 chunk，让它们真正按需加载。
    modulePreload: {
      resolveDependencies: (_filename, deps) =>
        deps.filter((dep) => {
          const normalized = dep.replace(/\\/g, "/");
          if (
            normalized.includes("/codemirror-")
            || normalized.includes("/mdxeditor-")
            || normalized.includes("/markdown-rendering-")
            || normalized.includes("/MarkdownEditorImpl-")
            || normalized.includes("/MarkdownPreviewImpl-")
            || normalized.includes("/CodeEditor-")
          ) {
            return false;
          }
          return true;
        }),
    },
    rollupOptions: {
      output: {
        manualChunks,
      },
    },
  },
  server: {
    port: 3000,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 3001,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
