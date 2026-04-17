import { memo, useCallback, useEffect, useState } from "react";
import { Sun, Moon, ArrowUpCircle, Github } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-shell";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { checkAppUpdate } from "../../lib/appUpdater";
import { showToast } from "../Toast";
import { t } from "../../lib/i18n";

function HeaderComponent() {
  const navigate = useNavigate();
  const [appUpdateAvailable, setAppUpdateAvailable] = useState(false);
  const [latestVersion, setLatestVersion] = useState<string | null>(null);
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme());

  useEffect(() => {
    void checkAppUpdate()
      .then(({ result }) => {
        setAppUpdateAvailable(result.update_available);
        if (result.update_available && result.latest_version) {
          setLatestVersion(result.latest_version);
          const labels = t();
          showToast("info", `${labels.settings.updateAvailable}: v${result.latest_version}`, 8000);
        }
      })
      .catch((err) => {
        console.warn("[cchub] update check failed:", err);
      });
  }, []);

  const toggleTheme = useCallback(() => {
    const next = currentTheme === "dark" ? "light" : "dark";
    setTheme(next);
    setCurrentTheme(next);
  }, [currentTheme]);

  const handleOpenGithub = useCallback(() => {
    void open("https://github.com/Moresl/cchub");
  }, []);

  return (
    <header className="topbar">
      <button
        className="theme-btn"
        title="GitHub"
        onClick={handleOpenGithub}
      >
        <Github size={16} />
      </button>

      <button className="theme-btn" onClick={toggleTheme} title="Toggle theme">
        {currentTheme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
      </button>

      {appUpdateAvailable && (
        <button
          className="badge badge-accent"
          style={{ gap: 6, cursor: "pointer", border: "none", background: "var(--accent-subtle)" }}
          title={`${t().settings.updateAvailable}${latestVersion ? `: v${latestVersion}` : ""}`}
          onClick={() => navigate("/settings")}
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
