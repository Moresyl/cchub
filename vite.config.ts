import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

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
  build: {
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
