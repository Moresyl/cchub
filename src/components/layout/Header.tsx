import { memo, useCallback } from "react";
import {
  ArrowLeft,
  ArrowRight,
  ArrowUpCircle,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  Sun,
} from "lucide-react";
import { useLocation } from "react-router-dom";
import { getResolvedTheme, setTheme } from "../../lib/theme";
import { getLocale, t } from "../../lib/i18n";
import { getNavigationSection } from "../../lib/navigation";
import { usePreferences } from "../../stores/preferences";
import ProjectProfileSwitcher from "../ProjectProfileSwitcher";
import { useAppUpdate } from "../AppUpdateHost";
import { Button } from "../ui/button";
import WindowControls, { detectDesktopPlatform } from "./WindowControls";

interface HeaderProps {
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}

function HeaderComponent({ sidebarCollapsed, onToggleSidebar }: HeaderProps) {
  const location = useLocation();
  const { updateAvailable, latestVersion, openUpdateDialog } = useAppUpdate();
  const currentTheme = usePreferences((state) => state.theme);
  const i = t();
  const locale = getLocale();
  const section = getNavigationSection(location.pathname);
  const item = section.items.find((candidate) => candidate.path === location.pathname) ?? section.items[0];
  const pageTitle = i.nav[item.labelKey];
  const sectionTitle = section.key === "settings" ? i.nav.settings : i.navGroups[section.key];
  const platform = detectDesktopPlatform(navigator.userAgent);
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);

  const toggleTheme = useCallback(() => {
    const next = getResolvedTheme() === "dark" ? "light" : "dark";
    setTheme(next);
  }, []);

  return (
    <header className={`desktop-titlebar platform-${platform}`} data-tauri-drag-region>
      <div
        className={`titlebar-sidebar-zone ${sidebarCollapsed ? "titlebar-sidebar-collapsed" : ""}`}
        data-tauri-drag-region
      >
        <div className="titlebar-history-actions">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => window.history.back()}
            aria-label={text("后退", "Back", "戻る")}
            title={text("后退", "Back", "戻る")}
          >
            <ArrowLeft size={15} aria-hidden="true" />
          </Button>
          {!sidebarCollapsed && (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => window.history.forward()}
              aria-label={text("前进", "Forward", "進む")}
              title={text("前进", "Forward", "進む")}
            >
              <ArrowRight size={15} aria-hidden="true" />
            </Button>
          )}
        </div>
        <div className="titlebar-sidebar-drag" data-tauri-drag-region />
        <Button
          variant="ghost"
          size="icon"
          className="titlebar-sidebar-toggle"
          onClick={onToggleSidebar}
          aria-label={
            sidebarCollapsed
              ? text("展开侧栏", "Expand sidebar", "サイドバーを展開")
              : text("折叠侧栏", "Collapse sidebar", "サイドバーを折りたたむ")
          }
          title={
            sidebarCollapsed
              ? text("展开侧栏", "Expand sidebar", "サイドバーを展開")
              : text("折叠侧栏", "Collapse sidebar", "サイドバーを折りたたむ")
          }
        >
          {sidebarCollapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
        </Button>
      </div>

      <div className="titlebar-main-zone" data-tauri-drag-region>
        <div className="titlebar-page-identity" aria-label={`${sectionTitle} / ${pageTitle}`} data-tauri-drag-region>
          <span className="titlebar-page-name">{pageTitle}</span>
          {sectionTitle !== pageTitle && <span className="titlebar-section-name">{sectionTitle}</span>}
        </div>
        <div className="titlebar-drag-spacer" data-tauri-drag-region />
        <div className="topbar-actions">
          <ProjectProfileSwitcher />
          <Button
            variant="ghost"
            size="icon"
            aria-label={text("快速切换", "Quick switch", "クイック切り替え")}
            title={text("快速切换", "Quick switch", "クイック切り替え")}
            onClick={() => window.dispatchEvent(new CustomEvent("cchub-open-command-palette"))}
          >
            <Search aria-hidden="true" size={16} />
          </Button>

          <Button
            variant="ghost"
            size="icon"
            onClick={toggleTheme}
            aria-label={`${i.settings.theme}: ${getResolvedTheme() === "dark" ? i.settings.light : i.settings.dark}`}
            title={`${i.settings.theme}: ${getResolvedTheme() === "dark" ? i.settings.light : i.settings.dark}`}
          >
            {currentTheme === "system" ? (
              <Monitor size={16} />
            ) : currentTheme === "dark" ? (
              <Sun size={16} />
            ) : (
              <Moon size={16} />
            )}
          </Button>

          {updateAvailable && (
            <Button
              variant="secondary"
              size="icon"
              className="topbar-update-button"
              aria-label={`${i.settings.updateAvailable}${latestVersion ? `: v${latestVersion}` : ""}`}
              title={`${i.settings.updateAvailable}${latestVersion ? `: v${latestVersion}` : ""}`}
              onClick={openUpdateDialog}
            >
              <ArrowUpCircle size={15} aria-hidden="true" />
            </Button>
          )}
        </div>
        <WindowControls platform={platform} />
      </div>
    </header>
  );
}

export default memo(HeaderComponent);
