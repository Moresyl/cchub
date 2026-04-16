import { Profiler, lazy, Suspense, useCallback, useEffect, useState, type ComponentType, type ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, useLocation, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Settings2, X } from "lucide-react";
import Sidebar from "./components/layout/Sidebar";
import Header from "./components/layout/Header";
import ErrorBoundary from "./components/ErrorBoundary";
import CommandPalette from "./components/CommandPalette";
import { ToastContainer } from "./components/Toast";
import DeepLinkImportDialog from "./components/DeepLinkImportDialog";
import WelcomeDialog from "./components/WelcomeDialog";
import { getLocale, setLocale, type Locale } from "./lib/i18n";
import { getTheme, setTheme, type Theme } from "./lib/theme";
import type { EnvironmentConflict } from "./lib/appPreferences";
import { queryClient } from "./lib/queryClient";
import {
  fetchClaudeMdPageData,
  fetchLogsPageData,
  fetchMarketplaceLocalData,
  fetchMcpServersPageData,
  fetchProfilesPageData,
  fetchSkillsPageData,
  fetchToolsPageData,
  queryKeys,
} from "./hooks/queries";

const pageImports = {
  "/": () => import("./pages/Dashboard"),
  "/mcp-servers": () => import("./pages/McpServers"),
  "/mcp-clients": () => import("./pages/McpClients"),
  "/logs": () => import("./pages/Logs"),
  "/skills": () => import("./pages/Skills"),
  "/workflows": () => import("./pages/Workflows"),
  "/autopilot": () => import("./pages/Autopilot"),
  "/marketplace": () => import("./pages/Marketplace"),
  "/hooks": () => import("./pages/Hooks"),
  "/workspaces": () => import("./pages/Workspaces"),
  "/profiles": () => import("./pages/Profiles"),
  "/sessions": () => import("./pages/Sessions"),
  "/claude-md": () => import("./pages/ClaudeMd"),
  "/config-files": () => import("./pages/ConfigFiles"),
  "/tools": () => import("./pages/Tools"),
  "/security": () => import("./pages/Security"),
  "/settings": () => import("./pages/Settings"),
} as const;

type RoutePath = keyof typeof pageImports;

const routeComponents: ReadonlyArray<{
  path: RoutePath;
  Component: React.LazyExoticComponent<ComponentType>;
}> = (Object.keys(pageImports) as RoutePath[]).map((path) => ({
  path,
  Component: lazy(pageImports[path]),
}));

// App 模块加载后立即并发预加载所有页面 chunk，消除首次点击延迟
Object.values(pageImports).forEach((loader) => void loader());

// 空闲时预热后端数据缓存，不与首屏数据请求抢后端
{
  const preloadData = () => {
    const today = new Date().toISOString().slice(0, 10);
    const prefetchers: Array<{ key: readonly unknown[]; fn: () => Promise<unknown> }> = [
      { key: queryKeys.mcpServersPage, fn: fetchMcpServersPageData },
      { key: queryKeys.skillsPage, fn: fetchSkillsPageData },
      { key: queryKeys.profilesPage, fn: fetchProfilesPageData },
      { key: queryKeys.marketplaceLocal, fn: fetchMarketplaceLocalData },
      { key: queryKeys.toolsPage, fn: fetchToolsPageData },
      { key: queryKeys.claudeMdPage, fn: fetchClaudeMdPageData },
      { key: queryKeys.logsPage(today), fn: () => fetchLogsPageData(today) },
    ];
    // 串行而非并行，避免一次性打爆后端
    prefetchers.reduce<Promise<unknown>>(
      (acc, { key, fn }) => acc.then(() => queryClient.prefetchQuery({ queryKey: key, queryFn: fn }).catch(() => {})),
      Promise.resolve(),
    );
  };
  if (typeof window.requestIdleCallback === "function") {
    window.requestIdleCallback(preloadData, { timeout: 5000 });
  } else {
    setTimeout(preloadData, 2500);
  }
}

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


function ActiveRoute({ pathname }: { pathname: string }) {
  const route = routeComponents.find((r) => r.path === pathname);
  if (!route) return null;
  const { Component } = route;
  return (
    <Suspense fallback={<RouteFallback />}>
      <Component />
    </Suspense>
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
  const [locale, setLocaleState] = useState<Locale>(getLocale());
  const [welcomeOpen, setWelcomeOpen] = useState(false);
  const [welcomeTheme, setWelcomeTheme] = useState<Theme>(getTheme());
  const [installedToolCount, setInstalledToolCount] = useState(0);
  const [profileCount, setProfileCount] = useState(0);
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  useEffect(() => {
    void loadEnvConflicts();
    void loadWelcomeState();
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

  async function loadWelcomeState() {
    try {
      const completed = await invoke<boolean>("get_welcome_completed");
      if (completed) {
        setWelcomeOpen(false);
        return;
      }
      await invoke("sync_config_profiles");
      const [tools, profiles] = await Promise.all([
        invoke<Array<{ installed: boolean }>>("detect_tools"),
        invoke<Array<unknown>>("get_config_profiles"),
      ]);
      setInstalledToolCount(tools.filter((tool) => tool.installed).length);
      setProfileCount(profiles.length);
      setWelcomeOpen(true);
    } catch {
      setWelcomeOpen(false);
    }
  }

  const handleWelcomeLocaleChange = useCallback((nextLocale: Locale) => {
    setLocale(nextLocale);
    setLocaleState(nextLocale);
  }, []);

  const handleWelcomeThemeChange = useCallback((nextTheme: Theme) => {
    setTheme(nextTheme);
    setWelcomeTheme(nextTheme);
  }, []);

  const handleWelcomeFinish = useCallback(async () => {
    try {
      await invoke("set_welcome_completed", { completed: true });
      setWelcomeOpen(false);
    } catch (error) {
      console.error(error);
    }
  }, []);

  const highlightVariables = Array.from(new Set(envConflicts.flatMap((item) => item.variables))).slice(0, 4);
  const showConflictBanner = envConflicts.length > 0 && !bannerDismissed && location.pathname !== "/settings";

  return (
    <>
      <ToastContainer />
      <DeepLinkImportDialog />
      <WelcomeDialog
        open={welcomeOpen}
        locale={locale}
        theme={welcomeTheme}
        installedToolCount={installedToolCount}
        profileCount={profileCount}
        onSelectLocale={handleWelcomeLocaleChange}
        onSelectTheme={handleWelcomeThemeChange}
        onFinish={() => void handleWelcomeFinish()}
      />
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
                <ActiveRoute pathname={location.pathname} />
              </RouteProfiler>
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </>
  );
}

export default App;
