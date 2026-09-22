import { ArrowRightLeft, FileJson2, Plug, Settings, Sparkles, type LucideIcon } from "lucide-react";
import type { RoutePath } from "./routes";

export type NavigationSectionKey = "overview" | "ecosystem" | "settings";

export interface NavigationItem {
  path: RoutePath;
  labelKey: "profiles" | "configFiles" | "mcpServers" | "skills" | "settings";
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
      { path: "/skills", labelKey: "skills", icon: Sparkles },
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
