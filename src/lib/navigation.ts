import {
  Activity,
  ArrowRightLeft,
  Bot,
  FileJson2,
  History,
  Layers3,
  Plug,
  ScrollText,
  Settings,
  ShieldCheck,
  Sparkles,
  type LucideIcon,
} from "lucide-react";
import type { RoutePath } from "./routes";

export type NavigationSectionKey = "overview" | "ecosystem" | "operations" | "settings";

export interface NavigationItem {
  path: RoutePath;
  labelKey:
    | "profiles"
    | "configFiles"
    | "mcpServers"
    | "prompts"
    | "skills"
    | "openClaw"
    | "mcode"
    | "proxyAdvanced"
    | "usage"
    | "sessions"
    | "settings";
  icon: LucideIcon;
}

export interface NavigationSection {
  key: NavigationSectionKey;
  icon: LucideIcon;
  defaultPath: RoutePath;
  items: readonly NavigationItem[];
}

export const navigationSections: readonly NavigationSection[] = [
  {
    key: "overview",
    icon: ArrowRightLeft,
    defaultPath: "/",
    items: [{ path: "/", labelKey: "profiles", icon: ArrowRightLeft }],
  },
  {
    key: "ecosystem",
    icon: FileJson2,
    defaultPath: "/config-files",
    items: [
      { path: "/config-files", labelKey: "configFiles", icon: FileJson2 },
      { path: "/mcp-servers", labelKey: "mcpServers", icon: Plug },
      { path: "/prompts", labelKey: "prompts", icon: ScrollText },
      { path: "/skills", labelKey: "skills", icon: Sparkles },
      { path: "/openclaw", labelKey: "openClaw", icon: Bot },
      { path: "/minimax-code", labelKey: "mcode", icon: Layers3 },
    ],
  },
  {
    key: "operations",
    icon: Activity,
    defaultPath: "/proxy-advanced",
    items: [
      { path: "/proxy-advanced", labelKey: "proxyAdvanced", icon: ShieldCheck },
      { path: "/usage", labelKey: "usage", icon: Activity },
      { path: "/sessions", labelKey: "sessions", icon: History },
    ],
  },
  {
    key: "settings",
    icon: Settings,
    defaultPath: "/settings",
    items: [{ path: "/settings", labelKey: "settings", icon: Settings }],
  },
] as const;

export function getNavigationSection(pathname: string): NavigationSection {
  return (
    navigationSections.find((section) => section.items.some((item) => item.path === pathname)) ?? navigationSections[0]
  );
}
