import { memo, useCallback, useEffect, useRef } from "react";
import { Command, PanelLeftClose, PanelLeftOpen, Settings } from "lucide-react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { getLocale, t } from "../../lib/i18n";
import { getNavigationSection, navigationSections } from "../../lib/navigation";
import { preloadRoute } from "../../lib/routes";
import { Button } from "../ui/button";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

function SidebarComponent({ collapsed, onToggle }: SidebarProps) {
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

  return (
    <aside className={`sidebar-shell ${collapsed ? "sidebar-collapsed" : ""}`} aria-label={i.app.name}>
      <div className="sidebar-brand">
        <button className="sidebar-brand-link" type="button" onClick={() => navigate("/")} aria-label={i.app.name}>
          <span className="sidebar-mark">C</span>
          <span className="sidebar-brand-name">CCHub</span>
        </button>
        <Button
          variant="ghost"
          size="icon"
          className="sidebar-collapse-button"
          onClick={onToggle}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
        </Button>
      </div>

      <Button
        variant="ghost"
        className="sidebar-command"
        title={locale === "zh" ? "快速切换" : "Quick switch"}
        aria-label={locale === "zh" ? "快速切换" : "Quick switch"}
        onClick={() => window.dispatchEvent(new CustomEvent("cchub-open-command-palette"))}
      >
        <Command size={15} aria-hidden="true" />
        <span>{locale === "zh" ? "快速切换" : locale === "ja" ? "クイック切替" : "Quick switch"}</span>
        <kbd>Ctrl K</kbd>
      </Button>

      <nav className="sidebar-context-nav" aria-label={i.app.name}>
        {navigationSections.slice(0, -1).map((section) => (
          <div className="sidebar-nav-group" key={section.key}>
            <div className="sidebar-nav-label">
              {section.key === "overview"
                ? locale === "zh"
                  ? "工作区"
                  : locale === "ja"
                    ? "ワークスペース"
                    : "Workspace"
                : locale === "zh"
                  ? "配置管理"
                  : locale === "ja"
                    ? "設定管理"
                    : "Configuration"}
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
                  <Icon size={16} aria-hidden="true" />
                  <span>{i.nav[item.labelKey]}</span>
                </NavLink>
              );
            })}
          </div>
        ))}
      </nav>

      <footer className="sidebar-context-footer">
        <NavLink
          to="/settings"
          className={({ isActive }) => `sidebar-context-link ${isActive ? "active" : ""}`}
          aria-current={activeSection.key === "settings" ? "page" : undefined}
          title={i.nav.settings}
          aria-label={i.nav.settings}
        >
          <Settings size={16} aria-hidden="true" />
          <span>{i.nav.settings}</span>
        </NavLink>
        <span className="sidebar-version">CCHub · v{__APP_VERSION__}</span>
      </footer>
    </aside>
  );
}

export default memo(SidebarComponent);
