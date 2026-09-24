import { memo, useCallback, useEffect, useMemo, useRef, type PointerEvent as ReactPointerEvent } from "react";
import { Blocks, CircleUserRound, FileJson2, Folder, Gauge, Hash, Plus, Search, Settings } from "lucide-react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { getLocale, t } from "../../lib/i18n";
import { getNavigationSection, navigationSections, type NavigationItem } from "../../lib/navigation";
import { preloadRoute } from "../../lib/routes";
import { DEFAULT_SIDEBAR_WIDTH } from "../../lib/sidebarWidth";
import { Button } from "../ui/button";

interface SidebarProps {
  collapsed: boolean;
  width: number;
  onResize: (width: number) => void;
}

const CONFIG_ITEMS: readonly NavigationItem[] = [
  ...navigationSections[0].items,
  ...navigationSections[1].items.filter((item) => item.path !== "/config-files" && item.path !== "/skills"),
];
const OPERATION_ITEMS = navigationSections[2].items;

function SidebarComponent({ collapsed, width, onResize }: SidebarProps) {
  const i = t();
  const location = useLocation();
  const navigate = useNavigate();
  const activeSection = getNavigationSection(location.pathname);
  const locale = getLocale();
  const hoverTimerRef = useRef<number | null>(null);
  const operationMode = OPERATION_ITEMS.some((item) => item.path === location.pathname);
  const visibleItems = useMemo(() => (operationMode ? OPERATION_ITEMS : CONFIG_ITEMS), [operationMode]);

  const text = useCallback(
    (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en),
    [locale],
  );

  const cancelHover = useCallback(() => {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
  }, []);

  const prefetchRoute = useCallback(
    (path: string) => {
      cancelHover();
      hoverTimerRef.current = window.setTimeout(() => {
        hoverTimerRef.current = null;
        preloadRoute(path);
      }, 160);
    },
    [cancelHover],
  );

  useEffect(() => cancelHover, [cancelHover]);

  const handleResizeMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
    onResize(event.clientX - event.currentTarget.closest("aside")!.getBoundingClientRect().left);
  };

  const handleResizeKey = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const delta = event.shiftKey ? 32 : 8;
    if (event.key === "ArrowLeft") onResize(width - delta);
    else if (event.key === "ArrowRight") onResize(width + delta);
    else if (event.key === "Home") onResize(DEFAULT_SIDEBAR_WIDTH);
    else return;
    event.preventDefault();
  };

  const handleCreateProfile = () => {
    if (location.pathname !== "/") navigate("/");
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("cchub-shortcut-new")), 60);
  };

  const primaryActions = [
    {
      key: "new",
      label: text("新增配置", "New configuration", "設定を追加"),
      shortcut: "Ctrl N",
      icon: Plus,
      onClick: handleCreateProfile,
    },
    {
      key: "search",
      label: text("搜索", "Search", "検索"),
      shortcut: "Ctrl K",
      icon: Search,
      onClick: () => window.dispatchEvent(new CustomEvent("cchub-open-command-palette")),
    },
    {
      key: "files",
      label: i.nav.configFiles,
      icon: FileJson2,
      onClick: () => navigate("/config-files"),
    },
    {
      key: "extensions",
      label: i.nav.skills,
      icon: Blocks,
      onClick: () => navigate("/skills"),
    },
  ];

  return (
    <aside className={`sidebar-shell ${collapsed ? "sidebar-collapsed" : ""}`} aria-label={i.app.name}>
      <div className="sidebar-primary-actions">
        {primaryActions.map((action) => {
          const Icon = action.icon;
          return (
            <Button
              key={action.key}
              variant="ghost"
              className="sidebar-primary-action"
              title={action.label}
              aria-label={action.label}
              onClick={action.onClick}
            >
              <Icon size={15} aria-hidden="true" />
              <span>{action.label}</span>
              {action.shortcut && <kbd>{action.shortcut}</kbd>}
            </Button>
          );
        })}
      </div>

      <nav className="sidebar-context-nav" aria-label={i.app.name}>
        <div className="sidebar-view-toolbar">
          <div
            className="sidebar-view-tabs"
            role="tablist"
            aria-label={text("导航视图", "Navigation view", "ナビゲーション表示")}
          >
            <Button
              variant="ghost"
              size="sm"
              className={!operationMode ? "active" : ""}
              role="tab"
              aria-selected={!operationMode}
              onClick={() => navigate("/")}
            >
              <Hash size={12} />
              {text("配置", "Configs", "設定")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className={operationMode ? "active" : ""}
              role="tab"
              aria-selected={operationMode}
              onClick={() => navigate("/proxy-advanced")}
            >
              <Gauge size={12} />
              {text("运行", "Runtime", "実行")}
            </Button>
          </div>
        </div>

        <div className="sidebar-tree-section">
          <div className="sidebar-tree-heading">
            <span>
              {operationMode
                ? text("运行与分析", "Runtime & analytics", "実行と分析")
                : text("客户端", "Clients", "クライアント")}
            </span>
          </div>
          <div className="sidebar-tree-list">
            {visibleItems.map((item, index) => {
              const Icon = item.icon;
              return (
                <NavLink
                  key={item.path}
                  to={item.path}
                  end={item.path === "/"}
                  onPointerEnter={() => prefetchRoute(item.path)}
                  onPointerLeave={cancelHover}
                  onFocus={() => preloadRoute(item.path)}
                  onClick={(event) => {
                    cancelHover();
                    if (event.currentTarget.classList.contains("active")) event.preventDefault();
                  }}
                  className={({ isActive }) => `sidebar-context-link ${isActive ? "active" : ""}`}
                  title={i.nav[item.labelKey]}
                  aria-label={i.nav[item.labelKey]}
                >
                  {index === 0 && !operationMode ? (
                    <Folder size={15} aria-hidden="true" />
                  ) : (
                    <Icon size={15} aria-hidden="true" />
                  )}
                  <span>{i.nav[item.labelKey]}</span>
                </NavLink>
              );
            })}
          </div>
        </div>
      </nav>

      <footer className="sidebar-context-footer">
        <div className="sidebar-product" title={`CCHub v${__APP_VERSION__}`}>
          <span className="sidebar-product-mark">
            <CircleUserRound size={15} aria-hidden="true" />
          </span>
          <span className="sidebar-product-name">CCHub</span>
        </div>
        <NavLink
          to="/settings"
          className={({ isActive }) => `sidebar-settings-link ${isActive ? "active" : ""}`}
          aria-current={activeSection.key === "settings" ? "page" : undefined}
          title={i.nav.settings}
          aria-label={i.nav.settings}
        >
          <Settings size={16} aria-hidden="true" />
        </NavLink>
      </footer>
      {!collapsed && (
        <div
          className="sidebar-resize-handle"
          role="separator"
          tabIndex={0}
          aria-label={text("调整侧栏宽度", "Resize sidebar", "サイドバーの幅を変更")}
          aria-orientation="vertical"
          aria-valuenow={width}
          aria-valuemin={264}
          aria-valuemax={360}
          onPointerDown={(event: ReactPointerEvent<HTMLDivElement>) =>
            event.currentTarget.setPointerCapture(event.pointerId)
          }
          onPointerMove={handleResizeMove}
          onPointerUp={(event: ReactPointerEvent<HTMLDivElement>) =>
            event.currentTarget.releasePointerCapture(event.pointerId)
          }
          onDoubleClick={() => onResize(DEFAULT_SIDEBAR_WIDTH)}
          onKeyDown={handleResizeKey}
        />
      )}
    </aside>
  );
}

export default memo(SidebarComponent);
