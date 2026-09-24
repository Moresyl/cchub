import { memo, useCallback } from "react";
import { Sun, Moon, Monitor, ArrowUpCircle, Search, ChevronRight } from "lucide-react";
import { useLocation } from "react-router-dom";
import { getResolvedTheme, setTheme } from "../../lib/theme";
import { getLocale, t } from "../../lib/i18n";
import { getNavigationSection } from "../../lib/navigation";
import { usePreferences } from "../../stores/preferences";
import ProjectProfileSwitcher from "../ProjectProfileSwitcher";
import { useAppUpdate } from "../AppUpdateHost";
import { Button } from "../ui/button";

function HeaderComponent() {
  const location = useLocation();
  const { updateAvailable, latestVersion, openUpdateDialog } = useAppUpdate();
  const currentTheme = usePreferences((state) => state.theme);
  const i = t();
  const locale = getLocale();
  const section = getNavigationSection(location.pathname);
  const item = section.items.find((candidate) => candidate.path === location.pathname) ?? section.items[0];
  const pageTitle = i.nav[item.labelKey];
  const sectionTitle = section.key === "settings" ? i.nav.settings : i.navGroups[section.key];

  const toggleTheme = useCallback(() => {
    const next = getResolvedTheme() === "dark" ? "light" : "dark";
    setTheme(next);
  }, []);

  return (
    <header className="topbar">
      <div className="topbar-title" aria-label={`${sectionTitle} / ${pageTitle}`}>
        {sectionTitle !== pageTitle && (
          <>
            <span className="topbar-section">{sectionTitle}</span>
            <ChevronRight size={14} aria-hidden="true" />
          </>
        )}
        <span className="topbar-page">{pageTitle}</span>
      </div>
      <div className="topbar-actions">
        <ProjectProfileSwitcher />
        <Button
          variant="ghost"
          size="icon"
          aria-label={locale === "zh" ? "快速切换" : "Quick switch"}
          title={locale === "zh" ? "快速切换" : "Quick switch"}
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
            size="sm"
            title={`${i.settings.updateAvailable}${latestVersion ? `: v${latestVersion}` : ""}`}
            onClick={openUpdateDialog}
          >
            <ArrowUpCircle size={13} />
            <span>{i.settings.updateAvailable}</span>
          </Button>
        )}
      </div>
    </header>
  );
}

export default memo(HeaderComponent);
