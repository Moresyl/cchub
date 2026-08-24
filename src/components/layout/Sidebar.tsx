import { memo, useCallback, useEffect, useRef } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { t } from "../../lib/i18n";
import { getNavigationSection, navigationSections, type NavigationSectionKey } from "../../lib/navigation";
import { preloadRoute } from "../../lib/routes";

function SidebarComponent() {
  const i = t();
  const location = useLocation();
  const navigate = useNavigate();
  const activeSection = getNavigationSection(location.pathname);
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

  const sectionLabel = (key: NavigationSectionKey) => (key === "settings" ? i.nav.settings : i.navGroups[key]);

  return (
    <aside className="sidebar-shell" aria-label={i.app.name}>
      <div className="sidebar-rail">
        <button className="sidebar-mark" type="button" onClick={() => navigate("/")} aria-label={i.nav.dashboard}>
          CC
        </button>

        <nav className="sidebar-rail-nav" aria-label={i.app.subtitle}>
          {navigationSections.slice(0, -1).map((section) => {
            const Icon = section.icon;
            const active = activeSection.key === section.key;
            return (
              <button
                key={section.key}
                type="button"
                className={`sidebar-rail-item ${active ? "active" : ""}`}
                aria-current={active ? "page" : undefined}
                title={sectionLabel(section.key)}
                onPointerEnter={() => prefetchRoute(section.defaultPath)}
                onPointerLeave={cancelHover}
                onFocus={() => preloadRoute(section.defaultPath)}
                onClick={() => navigate(section.defaultPath)}
              >
                <Icon size={17} aria-hidden="true" />
                <span>{sectionLabel(section.key)}</span>
              </button>
            );
          })}
        </nav>

        <div className="sidebar-rail-footer">
          {navigationSections.slice(-1).map((section) => {
            const Icon = section.icon;
            const active = activeSection.key === section.key;
            return (
              <button
                key={section.key}
                type="button"
                className={`sidebar-rail-item ${active ? "active" : ""}`}
                aria-current={active ? "page" : undefined}
                title={sectionLabel(section.key)}
                onClick={() => navigate(section.defaultPath)}
              >
                <Icon size={17} aria-hidden="true" />
                <span>{sectionLabel(section.key)}</span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="sidebar-context">
        <header className="sidebar-context-header">
          <div className="sidebar-context-eyebrow">{i.app.name}</div>
          <h2>{sectionLabel(activeSection.key)}</h2>
          <p>{i.app.subtitle}</p>
        </header>

        <nav className="sidebar-context-nav" aria-label={sectionLabel(activeSection.key)}>
          {activeSection.items.map((item) => {
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
              >
                <Icon size={15} aria-hidden="true" />
                <span>{i.nav[item.labelKey]}</span>
              </NavLink>
            );
          })}
        </nav>

        <footer className="sidebar-context-footer">
          <span className="sidebar-version-dot" aria-hidden="true" />
          <span>v{__APP_VERSION__}</span>
        </footer>
      </div>
    </aside>
  );
}

export default memo(SidebarComponent);
