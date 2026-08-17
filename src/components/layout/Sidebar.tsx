import { memo, useCallback, useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  Plug,
  Zap,
  Webhook,
  Settings,
  Shield,
  Store,
  Monitor,
  Activity,
  BarChart3,
  Layers,
  ArrowRightLeft,
  Wrench,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Bot,
  Brain,
  Terminal,
  MessageSquareText,
} from "lucide-react";
import { t } from "../../lib/i18n";
import {
  fetchClaudeMdPageData,
  fetchMarketplaceCatalogPage,
  fetchMarketplaceLocalData,
  fetchMarketplaceSearchPage,
  fetchMcpServersPageData,
  fetchProfilesPageData,
  fetchSessionsPageData,
  fetchSkillsPageData,
  fetchToolsPageData,
  queryKeys,
} from "../../hooks/queries";
import { preloadRoute } from "../../lib/routes";
import type { LucideIcon } from "lucide-react";

const navItems = [
  { path: "/", key: "dashboard" as const, icon: LayoutDashboard },
  { path: "/mcp-servers", key: "mcpServers" as const, icon: Plug },
  { path: "/mcp-clients", key: "mcpClients" as const, icon: Monitor },
  { path: "/logs", key: "logs" as const, icon: Activity },
  { path: "/usage", key: "usage" as const, icon: BarChart3 },
  { path: "/prompts", key: "prompts" as const, icon: MessageSquareText },
  { path: "/skills", key: "skills" as const, icon: Zap },
  { path: "/workflows", key: "workflows" as const, icon: GitBranch },
  { path: "/autopilot", key: "autopilot" as const, icon: Bot },
  { path: "/marketplace", key: "marketplace" as const, icon: Store },
  { path: "/hooks", key: "hooks" as const, icon: Webhook },
  { path: "/workspaces", key: "workspaces" as const, icon: Layers },
  { path: "/profiles", key: "profiles" as const, icon: ArrowRightLeft },
  { path: "/sessions", key: "sessions" as const, icon: History },
  { path: "/hermes-memory", key: "hermesMemory" as const, icon: Brain },
  { path: "/hermes-providers", key: "hermesProviders" as const, icon: Brain },
  { path: "/openclaw", key: "openClaw" as const, icon: Terminal },
  { path: "/proxy-advanced", key: "proxyAdvanced" as const, icon: ArrowRightLeft },
  { path: "/claude-md", key: "claudeMd" as const, icon: FileText },
  { path: "/config-files", key: "configFiles" as const, icon: FolderOpen },
  { path: "/tools", key: "tools" as const, icon: Wrench },
  { path: "/security", key: "security" as const, icon: Shield },
  { path: "/settings", key: "settings" as const, icon: Settings },
];

interface SidebarNavItemProps {
  path: string;
  label: string;
  icon: LucideIcon;
  onPrefetch: (path: string, priority: "hover" | "focus") => void;
  onPrefetchCancel: () => void;
}

function SidebarNavItemComponent({ path, label, icon: Icon, onPrefetch, onPrefetchCancel }: SidebarNavItemProps) {
  return (
    <NavLink
      to={path}
      end={path === "/"}
      onClick={(event) => {
        onPrefetchCancel();
        if (event.currentTarget.classList.contains("active")) {
          event.preventDefault();
        }
      }}
      onPointerEnter={() => onPrefetch(path, "hover")}
      onPointerLeave={onPrefetchCancel}
      onFocus={() => onPrefetch(path, "focus")}
      onBlur={onPrefetchCancel}
      className={({ isActive }) => `sidebar-link ${isActive ? "active" : ""}`}
    >
      <Icon size={15} />
      <span>{label}</span>
    </NavLink>
  );
}

const SidebarNavItem = memo(SidebarNavItemComponent);

function SidebarComponent() {
  const i = t();
  const queryClient = useQueryClient();
  // 悬停节流：鼠标掠过不触发预取，停留 180ms 才开始
  const hoverTimerRef = useRef<number | null>(null);
  const cancelHover = useCallback(() => {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
  }, []);
  useEffect(() => cancelHover, [cancelHover]);

  const prefetchRouteData = useCallback(
    (path: string) => {
      const ignorePreloadFailure = (promise: Promise<unknown>) => {
        void promise.catch((error: unknown) => {
          console.debug("Route data preload failed", path, error);
        });
      };

      switch (path) {
        case "/mcp-servers":
          ignorePreloadFailure(
            queryClient.prefetchQuery({
              queryKey: queryKeys.mcpServersPage,
              queryFn: fetchMcpServersPageData,
            }),
          );
          return;
        case "/skills":
          ignorePreloadFailure(
            queryClient.prefetchQuery({
              queryKey: queryKeys.skillsPage,
              queryFn: fetchSkillsPageData,
            }),
          );
          return;
        case "/marketplace":
          ignorePreloadFailure(
            Promise.all([
              queryClient.prefetchQuery({
                queryKey: queryKeys.marketplaceLocal,
                queryFn: fetchMarketplaceLocalData,
              }),
              queryClient.prefetchQuery({
                queryKey: queryKeys.marketplaceCatalog(),
                queryFn: () => fetchMarketplaceCatalogPage(),
              }),
              queryClient.prefetchQuery({
                queryKey: queryKeys.marketplaceSearch("mcp server"),
                queryFn: () => fetchMarketplaceSearchPage("mcp server"),
              }),
            ]),
          );
          return;
        case "/profiles":
          ignorePreloadFailure(
            queryClient.prefetchQuery({
              queryKey: queryKeys.profilesPage,
              queryFn: fetchProfilesPageData,
            }),
          );
          return;
        case "/tools":
          ignorePreloadFailure(
            queryClient.prefetchQuery({
              queryKey: queryKeys.toolsPage,
              queryFn: fetchToolsPageData,
            }),
          );
          return;
        case "/claude-md":
          ignorePreloadFailure(
            queryClient.prefetchQuery({
              queryKey: queryKeys.claudeMdPage,
              queryFn: fetchClaudeMdPageData,
            }),
          );
          return;
        case "/sessions":
          ignorePreloadFailure(
            queryClient.prefetchQuery({
              queryKey: queryKeys.sessions(null),
              queryFn: () => fetchSessionsPageData(null),
            }),
          );
          return;
        default:
          return;
      }
    },
    [queryClient],
  );

  const prefetchRoute = useCallback(
    (path: string, priority: "hover" | "focus") => {
      cancelHover();
      // 模块下载不会占用 Tauri 数据库锁，可在出现导航意图时立即开始。
      preloadRoute(path);
      if (priority === "focus") {
        prefetchRouteData(path);
        return;
      }
      // 文件系统扫描和数据库查询延后到明确悬停，避免鼠标掠过侧栏时争抢 IPC。
      hoverTimerRef.current = window.setTimeout(() => {
        hoverTimerRef.current = null;
        prefetchRouteData(path);
      }, 650);
    },
    [cancelHover, prefetchRouteData],
  );

  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div
            style={{
              width: 28,
              height: 28,
              borderRadius: 4,
              background: "var(--text-primary)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--bg-app)",
              fontSize: 11,
              fontWeight: 800,
            }}
          >
            CC
          </div>
          <div>
            <div style={{ fontSize: 13, fontWeight: 700, color: "var(--text-primary)", letterSpacing: "-0.02em" }}>
              {i.app.name}
            </div>
            <div style={{ fontSize: 10, color: "var(--text-muted)", marginTop: 1 }}>{i.app.subtitle}</div>
          </div>
        </div>
      </div>
      <nav className="sidebar-nav">
        {navItems.map((item) => (
          <SidebarNavItem
            key={item.path}
            path={item.path}
            label={i.nav[item.key]}
            icon={item.icon}
            onPrefetch={prefetchRoute}
            onPrefetchCancel={cancelHover}
          />
        ))}
      </nav>
      <div className="sidebar-footer">
        <div
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 5,
            padding: "3px 10px 3px 8px",
            borderRadius: 20,
            background: "var(--bg-card)",
            border: "1px solid var(--border-default)",
          }}
        >
          <span
            style={{
              width: 5,
              height: 5,
              borderRadius: "50%",
              background: "#22c55e",
              boxShadow: "0 0 4px rgba(34, 197, 94, 0.5)",
            }}
          />
          <span
            style={{
              fontSize: 10,
              fontWeight: 500,
              color: "var(--text-secondary)",
              letterSpacing: "0.03em",
            }}
          >
            v{__APP_VERSION__}
          </span>
        </div>
      </div>
    </aside>
  );
}

export default memo(SidebarComponent);
