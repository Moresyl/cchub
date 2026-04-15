import { Profiler, lazy, Suspense, useCallback, useEffect, useState, type ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route, useLocation, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Settings2, X } from "lucide-react";
import Sidebar from "./components/layout/Sidebar";
import Header from "./components/layout/Header";
import ErrorBoundary from "./components/ErrorBoundary";
import CommandPalette from "./components/CommandPalette";
import { ToastContainer } from "./components/Toast";
import DeepLinkImportDialog from "./components/DeepLinkImportDialog";
import { getLocale } from "./lib/i18n";
import type { EnvironmentConflict } from "./lib/appPreferences";
import { queryClient } from "./lib/queryClient";

const Dashboard = lazy(() => import("./pages/Dashboard"));
const McpServers = lazy(() => import("./pages/McpServers"));
const McpClients = lazy(() => import("./pages/McpClients"));
const Logs = lazy(() => import("./pages/Logs"));
const Skills = lazy(() => import("./pages/Skills"));
const Workflows = lazy(() => import("./pages/Workflows"));
const Autopilot = lazy(() => import("./pages/Autopilot"));
const Hooks = lazy(() => import("./pages/Hooks"));
const Settings = lazy(() => import("./pages/Settings"));
const Security = lazy(() => import("./pages/Security"));
const Marketplace = lazy(() => import("./pages/Marketplace"));
const Workspaces = lazy(() => import("./pages/Workspaces"));
const Profiles = lazy(() => import("./pages/Profiles"));
const Sessions = lazy(() => import("./pages/Sessions"));
const Tools = lazy(() => import("./pages/Tools"));
const ClaudeMd = lazy(() => import("./pages/ClaudeMd"));
const ConfigFiles = lazy(() => import("./pages/ConfigFiles"));

function RouteFallback() {
  return (
    <div
      style={{
        height: "100%",
        borderRadius: 8,
        background: "var(--bg-card)",
        border: "1px solid var(--border-subtle)",
      }}
    />
  );
}

function RouteProfiler({
  children,
  pathname,
}: {
  children: ReactNode;
  pathname: string;
}) {
  const handleRender = useCallback(
    (_id: string, phase: string, actualDuration: number) => {
      if (!import.meta.env.DEV || phase !== "update") {
        return;
      }

      console.debug(
        `[route-profiler] ${pathname} commit ${actualDuration.toFixed(2)}ms`,
      );
    },
    [pathname],
  );

  return (
    <Profiler id={`route:${pathname}`} onRender={handleRender}>
      {children}
    </Profiler>
  );
}

function RouteContent({ location }: { location: ReturnType<typeof useLocation> }) {
  return (
    <div key={location.pathname} className="page-enter" style={{ height: "100%" }}>
      <Routes location={location}>
        <Route path="/" element={<Dashboard />} />
        <Route path="/mcp-servers" element={<McpServers />} />
        <Route path="/mcp-clients" element={<McpClients />} />
        <Route path="/logs" element={<Logs />} />
        <Route path="/skills" element={<Skills />} />
        <Route path="/workflows" element={<Workflows />} />
        <Route path="/autopilot" element={<Autopilot />} />
        <Route path="/marketplace" element={<Marketplace />} />
        <Route path="/hooks" element={<Hooks />} />
        <Route path="/workspaces" element={<Workspaces />} />
        <Route path="/profiles" element={<Profiles />} />
        <Route path="/sessions" element={<Sessions />} />
        <Route path="/claude-md" element={<ClaudeMd />} />
        <Route path="/config-files" element={<ConfigFiles />} />
        <Route path="/tools" element={<Tools />} />
        <Route path="/security" element={<Security />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </div>
  );
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AppShell />
      </BrowserRouter>
    </QueryClientProvider>
  );
}

function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const [envConflicts, setEnvConflicts] = useState<EnvironmentConflict[]>([]);
  const [bannerDismissed, setBannerDismissed] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const locale = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  useEffect(() => {
    void loadEnvConflicts();
    const handleFocus = () => void loadEnvConflicts();
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const isTextEditingTarget = Boolean(
        target?.closest("input, textarea, [contenteditable='true'], .cm-editor"),
      );

      if ((event.ctrlKey || event.metaKey) && event.key === ",") {
        event.preventDefault();
        navigate("/settings");
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandPaletteOpen(true);
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
        event.preventDefault();
        window.dispatchEvent(new CustomEvent("cchub-shortcut-save"));
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n") {
        event.preventDefault();
        window.dispatchEvent(new CustomEvent("cchub-shortcut-new"));
        return;
      }

      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
        if (isTextEditingTarget) return;
        event.preventDefault();
        window.dispatchEvent(new CustomEvent("cchub-shortcut-search"));
        return;
      }

      if (
        event.key === "Escape"
        && !event.altKey
        && !event.ctrlKey
        && !event.metaKey
        && !event.shiftKey
      ) {
        window.dispatchEvent(new CustomEvent("cchub-shortcut-escape"));
      }
    };

    window.addEventListener("focus", handleFocus);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [navigate]);

  useEffect(() => {
    if (envConflicts.length === 0) {
      setBannerDismissed(false);
    }
  }, [envConflicts.length]);

  useEffect(() => {
    setCommandPaletteOpen(false);
  }, [location.pathname]);

  async function loadEnvConflicts() {
    try {
      const conflicts = await invoke<EnvironmentConflict[]>("get_environment_conflicts");
      setEnvConflicts(conflicts);
    } catch {
      setEnvConflicts([]);
    }
  }

  const highlightVariables = Array.from(new Set(envConflicts.flatMap((item) => item.variables))).slice(0, 4);
  const showConflictBanner = envConflicts.length > 0 && !bannerDismissed && location.pathname !== "/settings";

  return (
    <>
      <ToastContainer />
      <DeepLinkImportDialog />
      <CommandPalette
        open={commandPaletteOpen}
        onOpenChange={setCommandPaletteOpen}
        navigate={navigate}
      />
      <div className="app-layout">
        <Sidebar />
        <div className="main-area">
          <Header />
          {showConflictBanner && (
            <div className="env-warning-banner">
              <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0 }}>
                <AlertTriangle size={16} style={{ color: "var(--warning)", flexShrink: 0 }} />
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontSize: 12, fontWeight: 700 }}>
                    {uiText(
                      `检测到 ${envConflicts.length} 项环境变量冲突`,
                      `${envConflicts.length} environment override warning(s) detected`,
                      `${envConflicts.length} 件の環境変数上書き警告を検出しました`,
                    )}
                  </div>
                  <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                    {uiText(
                      `这些变量可能覆盖 CCHub 的配置切换: ${highlightVariables.join(", ")}`,
                      `These variables may override CCHub-managed settings: ${highlightVariables.join(", ")}`,
                      `これらの変数により CCHub 管理設定が上書きされる可能性があります: ${highlightVariables.join(", ")}`,
                    )}
                  </div>
                </div>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexShrink: 0 }}>
                <button className="btn btn-secondary btn-xs" onClick={() => navigate("/settings")} style={{ gap: 5 }}>
                  <Settings2 size={12} />
                  {uiText("查看设置", "Open Settings", "設定を開く")}
                </button>
                <button className="btn btn-ghost btn-icon-sm" onClick={() => setBannerDismissed(true)} title={uiText("关闭", "Dismiss", "閉じる")}>
                  <X size={14} />
                </button>
              </div>
            </div>
          )}
          <main className="page-content">
            <ErrorBoundary resetKey={location.pathname}>
              <RouteProfiler pathname={location.pathname}>
                <Suspense fallback={<RouteFallback />}>
                  <RouteContent location={location} />
                </Suspense>
              </RouteProfiler>
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </>
  );
}

export default App;
