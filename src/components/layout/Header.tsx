import { memo, useCallback, useState } from "react";
import { Sun, Moon, ArrowUpCircle, Github } from "lucide-react";
import { open } from "@tauri-apps/plugin-shell";
import { useLocation } from "react-router-dom";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { getLocale, t } from "../../lib/i18n";
import { getNavigationSection } from "../../lib/navigation";
import ProjectProfileSwitcher from "../ProjectProfileSwitcher";
import { useAppUpdate } from "../AppUpdateHost";
import { Button } from "../ui/button";

function HeaderComponent() {
  const location = useLocation();
  const { updateAvailable, latestVersion, openUpdateDialog } = useAppUpdate();
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme());
  const i = t();
  const locale = getLocale();
  const section = getNavigationSection(location.pathname);
  const item = section.items.find((candidate) => candidate.path === location.pathname) ?? section.items[0];
  const pageTitle = i.nav[item.labelKey];
  const sectionTitle = section.key === "settings" ? i.nav.settings : i.navGroups[section.key];
  const pageSubtitle =
    locale === "zh"
      ? `${sectionTitle} · 集中管理、状态检查与快速操作`
      : locale === "ja"
        ? `${sectionTitle} · 一元管理、状態確認、クイック操作`
        : `${sectionTitle} · Centralized management, status, and quick actions`;

  const toggleTheme = useCallback(() => {
    const next = currentTheme === "dark" ? "light" : "dark";
    setTheme(next);
    setCurrentTheme(next);
  }, [currentTheme]);

  const handleOpenGithub = useCallback(() => {
    void open("https://github.com/Moresyl/cchub");
  }, []);

  return (
    <header className="topbar">
      <div className="topbar-title">
        <h1>{pageTitle}</h1>
        <p>{pageSubtitle}</p>
      </div>
      <div className="topbar-actions">
        <ProjectProfileSwitcher />
        <Button variant="ghost" size="icon" aria-label="GitHub" title="GitHub" onClick={handleOpenGithub}>
          <Github aria-hidden="true" size={16} />
        </Button>

        <Button
          variant="ghost"
          size="icon"
          onClick={toggleTheme}
          aria-label={`${i.settings.theme}: ${currentTheme === "dark" ? i.settings.light : i.settings.dark}`}
          title={`${i.settings.theme}: ${currentTheme === "dark" ? i.settings.light : i.settings.dark}`}
        >
          {currentTheme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
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

        <div className="dot dot-active" role="status" aria-label="Connected" title="Connected" />
      </div>
    </header>
  );
}

export default memo(HeaderComponent);
