import { memo, useCallback, useEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";
import { ArrowLeft, ArrowRight, PanelLeftClose, PanelLeftOpen, Plus, Search, Settings } from "lucide-react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { getLocale, t } from "../../lib/i18n";
import { getNavigationSection, navigationSections } from "../../lib/navigation";
import { preloadRoute } from "../../lib/routes";
import { Button } from "../ui/button";
import { DEFAULT_SIDEBAR_WIDTH } from "../../lib/sidebarWidth";
import appIcon from "../../../src-tauri/icons/128x128.png";

interface SidebarProps {
  collapsed: boolean;
  width: number;
  onResize: (width: number) => void;
  onToggle: () => void;
}

function SidebarComponent({ collapsed, width, onResize, onToggle }: SidebarProps) {
  const i = t();
  const location = useLocation();
  const navigate = useNavigate();
  const activeSection = getNavigationSection(location.pathname);
  const locale = getLocale();
  const hoverTimerRef = useRef<number | null>(null);

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

  return (
    <aside className={`sidebar-shell ${collapsed ? "sidebar-collapsed" : ""}`} aria-label={i.app.name}>
      <div className="sidebar-window-controls">
        {!collapsed && (
          <div className="sidebar-history-actions">
            <Button
              variant="ghost"
              size="icon"
              onClick={() => window.history.back()}
              aria-label={locale === "zh" ? "后退" : "Back"}
              title={locale === "zh" ? "后退" : "Back"}
            >
              <ArrowLeft size={15} />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => window.history.forward()}
              aria-label={locale === "zh" ? "前进" : "Forward"}
              title={locale === "zh" ? "前进" : "Forward"}
            >
              <ArrowRight size={15} />
            </Button>
          </div>
        )}
        <Button
          variant="ghost"
          size="icon"
          className="sidebar-collapse-button"
          onClick={onToggle}
          aria-label={
            collapsed
              ? locale === "zh"
                ? "展开侧栏"
                : "Expand sidebar"
              : locale === "zh"
                ? "折叠侧栏"
                : "Collapse sidebar"
          }
          title={
            collapsed
              ? locale === "zh"
                ? "展开侧栏"
                : "Expand sidebar"
              : locale === "zh"
                ? "折叠侧栏"
                : "Collapse sidebar"
          }
        >
          {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
        </Button>
      </div>

      <div className="sidebar-primary-actions">
        <Button
          variant="ghost"
          className="sidebar-primary-action"
          title={locale === "zh" ? "新增配置" : "New configuration"}
          aria-label={locale === "zh" ? "新增配置" : "New configuration"}
          onClick={handleCreateProfile}
        >
          <Plus size={15} aria-hidden="true" />
          <span>{locale === "zh" ? "新增配置" : locale === "ja" ? "設定を追加" : "New configuration"}</span>
          <kbd>Ctrl N</kbd>
        </Button>
        <Button
          variant="ghost"
          className="sidebar-primary-action"
          title={locale === "zh" ? "搜索" : "Search"}
          aria-label={locale === "zh" ? "搜索" : "Search"}
          onClick={() => window.dispatchEvent(new CustomEvent("cchub-open-command-palette"))}
        >
          <Search size={15} aria-hidden="true" />
          <span>{locale === "zh" ? "搜索" : locale === "ja" ? "検索" : "Search"}</span>
          <kbd>Ctrl K</kbd>
        </Button>
      </div>

      <nav className="sidebar-context-nav" aria-label={i.app.name}>
        {navigationSections.slice(0, -1).map((section) => (
          <div className="sidebar-nav-group" key={section.key}>
            <div className="sidebar-nav-label">
              {section.key === "settings" ? i.nav.settings : i.navGroups[section.key]}
            </div>
            {section.items.map((item) => {
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
                  <Icon size={15} aria-hidden="true" />
                  <span>{i.nav[item.labelKey]}</span>
                </NavLink>
              );
            })}
          </div>
        ))}
      </nav>

      <footer className="sidebar-context-footer">
        <div className="sidebar-product" title={`CCHub v${__APP_VERSION__}`}>
          <span className="sidebar-product-mark">
            <img src={appIcon} alt="" aria-hidden="true" />
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
          aria-label={locale === "zh" ? "调整侧栏宽度" : locale === "ja" ? "サイドバーの幅を調整" : "Resize sidebar"}
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
