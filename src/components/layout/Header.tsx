import { memo, useCallback, useState } from "react";
import { Sun, Moon, ArrowUpCircle, Github } from "lucide-react";
import { open } from "@tauri-apps/plugin-shell";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { t } from "../../lib/i18n";
import ProjectProfileSwitcher from "../ProjectProfileSwitcher";
import { useAppUpdate } from "../AppUpdateHost";

function HeaderComponent() {
  const { updateAvailable, latestVersion, openUpdateDialog } = useAppUpdate();
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme());

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
      <ProjectProfileSwitcher />
      <button className="theme-btn" title="GitHub" onClick={handleOpenGithub}>
        <Github size={16} />
      </button>

      <button className="theme-btn" onClick={toggleTheme} title="Toggle theme">
        {currentTheme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
      </button>

      {updateAvailable && (
        <button
          className="badge badge-accent"
          style={{ gap: 6, cursor: "pointer", border: "none", background: "var(--accent-subtle)" }}
          title={`${t().settings.updateAvailable}${latestVersion ? `: v${latestVersion}` : ""}`}
          onClick={openUpdateDialog}
        >
          <ArrowUpCircle size={12} />
          <span style={{ fontSize: 11 }}>{t().settings.updateAvailable}</span>
        </button>
      )}

      <div className="dot dot-active" title="Connected" />
    </header>
  );
}

export default memo(HeaderComponent);
