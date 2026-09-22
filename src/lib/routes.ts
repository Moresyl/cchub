export const pageImports = {
  "/": () => import("../pages/Profiles"),
  "/mcp-servers": () => import("../pages/McpServers"),
  "/prompts": () => import("../pages/Prompts"),
  "/skills": () => import("../pages/Skills"),
  "/sessions": () => import("../pages/Sessions"),
  "/usage": () => import("../pages/Usage"),
  "/proxy-advanced": () => import("../pages/ProxyAdvanced"),
  "/openclaw": () => import("../pages/OpenClaw"),
  "/minimax-code": () => import("../pages/Mcode"),
  "/config-files": () => import("../pages/ConfigFiles"),
  "/settings": () => import("../pages/Settings"),
} as const;

export type RoutePath = keyof typeof pageImports;

export function preloadRoute(path: string): void {
  if (!(path in pageImports)) return;
  void pageImports[path as RoutePath]().catch((error) => {
    console.debug("Route preload failed", path, error);
  });
}
