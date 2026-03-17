import { useState, useEffect } from "react";
import { Sun, Moon, ArrowUpCircle } from "lucide-react";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { checkAppUpdate } from "../../lib/appUpdater";

export default function Header() {
  const [appUpdateAvailable, setAppUpdateAvailable] = useState(false);
  const [currentTheme, setCurrentTheme] = useState<Theme>(getTheme());

  useEffect(() => {
    checkAppUpdate()
      .then(({ result }) => setAppUpdateAvailable(result.update_available))
      .catch(() => {});
  }, []);

  function toggleTheme() {
    const next = currentTheme === "dark" ? "light" : "dark";
    setTheme(next);
    setCurrentTheme(next);
  }

  return (
    <header className="topbar">
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
