import { memo, useCallback, useEffect, useRef } from "react";
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
import { preloadRoute } from "../../lib/routes";
import type { LucideIcon } from "lucide-react";

const navGroups = [
  {
    key: "overview" as const,
    items: [
      { path: "/", key: "dashboard" as const, icon: LayoutDashboard },
      { path: "/usage", key: "usage" as const, icon: BarChart3 },
      { path: "/sessions", key: "sessions" as const, icon: History },
      { path: "/logs", key: "logs" as const, icon: Activity },
    ],
  },
  {
    key: "ecosystem" as const,
    items: [
      { path: "/mcp-servers", key: "mcpServers" as const, icon: Plug },
      { path: "/mcp-clients", key: "mcpClients" as const, icon: Monitor },
      { path: "/skills", key: "skills" as const, icon: Zap },
      { path: "/marketplace", key: "marketplace" as const, icon: Store },
      { path: "/workspaces", key: "workspaces" as const, icon: Layers },
      { path: "/profiles", key: "profiles" as const, icon: ArrowRightLeft },
    ],
  },
  {
    key: "automation" as const,
    items: [
      { path: "/prompts", key: "prompts" as const, icon: MessageSquareText },
      { path: "/workflows", key: "workflows" as const, icon: GitBranch },
      { path: "/autopilot", key: "autopilot" as const, icon: Bot },
      { path: "/hooks", key: "hooks" as const, icon: Webhook },
    ],
  },
  {
    key: "advanced" as const,
    items: [
      { path: "/claude-md", key: "claudeMd" as const, icon: FileText },
      { path: "/config-files", key: "configFiles" as const, icon: FolderOpen },
      { path: "/tools", key: "tools" as const, icon: Wrench },
      { path: "/security", key: "security" as const, icon: Shield },
      { path: "/hermes-memory", key: "hermesMemory" as const, icon: Brain },
      { path: "/hermes-providers", key: "hermesProviders" as const, icon: Brain },
      { path: "/openclaw", key: "openClaw" as const, icon: Terminal },
      { path: "/proxy-advanced", key: "proxyAdvanced" as const, icon: ArrowRightLeft },
    ],
  },
];

interface SidebarNavItemProps {
  path: string;
  label: string;
  icon: LucideIcon;
  onPrefetch: (path: string) => void;
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
      onPointerEnter={() => onPrefetch(path)}
      onPointerLeave={onPrefetchCancel}
      onFocus={() => {
        onPrefetchCancel();
        preloadRoute(path);
      }}
      onBlur={onPrefetchCancel}
      className={({ isActive }) => `sidebar-link ${isActive ? "active" : ""}`}
    >
      <Icon aria-hidden="true" size={15} />
      <span>{label}</span>
    </NavLink>
  );
}

const SidebarNavItem = memo(SidebarNavItemComponent);

function SidebarComponent() {
  const i = t();
  // 仅预加载页面模块。数据扫描必须等用户真正进入页面后再执行，避免侧栏悬停争抢 IPC。
  const hoverTimerRef = useRef<number | null>(null);
  const cancelHover = useCallback(() => {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
  }, []);
  useEffect(() => cancelHover, [cancelHover]);

  const prefetchRoute = useCallback(
    (path: string) => {
      cancelHover();
      hoverTimerRef.current = window.setTimeout(() => {
        hoverTimerRef.current = null;
        preloadRoute(path);
      }, 180);
    },
    [cancelHover],
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
        {navGroups.map((group) => (
          <section className="sidebar-group" key={group.key} aria-labelledby={`sidebar-group-${group.key}`}>
            <h2 className="sidebar-group-label" id={`sidebar-group-${group.key}`}>
              {i.navGroups[group.key]}
            </h2>
            {group.items.map((item) => (
              <SidebarNavItem
                key={item.path}
                path={item.path}
                label={i.nav[item.key]}
                icon={item.icon}
                onPrefetch={prefetchRoute}
                onPrefetchCancel={cancelHover}
              />
            ))}
          </section>
        ))}
      </nav>
      <div className="sidebar-footer">
        <SidebarNavItem
          path="/settings"
          label={i.nav.settings}
          icon={Settings}
          onPrefetch={prefetchRoute}
          onPrefetchCancel={cancelHover}
        />
        <div
          className="sidebar-version"
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
