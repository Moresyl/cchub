export const pageImports = {
  "/": () => import("../pages/Dashboard"),
  "/mcp-servers": () => import("../pages/McpServers"),
  "/mcp-clients": () => import("../pages/McpClients"),
  "/logs": () => import("../pages/Logs"),
  "/usage": () => import("../pages/Usage"),
  "/prompts": () => import("../pages/Prompts"),
  "/skills": () => import("../pages/Skills"),
  "/workflows": () => import("../pages/Workflows"),
  "/autopilot": () => import("../pages/Autopilot"),
  "/marketplace": () => import("../pages/Marketplace"),
  "/hooks": () => import("../pages/Hooks"),
  "/workspaces": () => import("../pages/Workspaces"),
  "/profiles": () => import("../pages/Profiles"),
  "/sessions": () => import("../pages/Sessions"),
  "/hermes-memory": () => import("../pages/HermesMemory"),
  "/hermes-providers": () => import("../pages/HermesProviders"),
  "/openclaw": () => import("../pages/OpenClaw"),
  "/proxy-advanced": () => import("../pages/ProxyAdvanced"),
  "/claude-md": () => import("../pages/ClaudeMd"),
  "/config-files": () => import("../pages/ConfigFiles"),
  "/tools": () => import("../pages/Tools"),
  "/security": () => import("../pages/Security"),
  "/settings": () => import("../pages/Settings"),
} as const;

export type RoutePath = keyof typeof pageImports;

export function preloadRoute(path: string): void {
  if (!(path in pageImports)) return;
  void pageImports[path as RoutePath]().catch((error) => {
    console.debug("Route preload failed", path, error);
  });
}
