import { memo, useCallback, useEffect, useState } from "react";
import { Sun, Moon, ArrowUpCircle, Github } from "lucide-react";
import { open } from "@tauri-apps/plugin-shell";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { checkAppUpdate } from "../../lib/appUpdater";

function HeaderComponent() {
  const [appUpdateAvailable, setAppUpdateAvailable] = useState(false);
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme());

  useEffect(() => {
    void checkAppUpdate()
      .then(({ result }) => setAppUpdateAvailable(result.update_available))
      .catch(() => {});
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
        <div className="badge badge-accent" style={{ gap: 6 }} title="App update available">
          <ArrowUpCircle size={12} />
        </div>
      )}

      <div className="dot dot-active" title="Connected" />
    </header>
  );
}

export default memo(HeaderComponent);
