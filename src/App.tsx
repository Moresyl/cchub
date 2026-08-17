import {
  Profiler,
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  useState,
  type ComponentType,
  type ReactNode,
} from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, useLocation, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AlertTriangle, Settings2, X } from "lucide-react";
import Sidebar from "./components/layout/Sidebar";
import Header from "./components/layout/Header";
import ErrorBoundary from "./components/ErrorBoundary";
import { showToast, ToastContainer } from "./components/Toast";
import DeepLinkImportHost from "./components/DeepLinkImportHost";
import NavigationProgress from "./components/NavigationProgress";
import AppUpdateHost from "./components/AppUpdateHost";

// CommandPalette 仅在 Ctrl+K 时显示，懒加载避免 cmdk 进入主 bundle。
const CommandPalette = lazy(() => import("./components/CommandPalette"));
const WelcomeDialog = lazy(() => import("./components/WelcomeDialog"));
import { getLocale, setLocale, type Locale } from "./lib/i18n";
import { getTheme, setTheme, type Theme } from "./lib/theme";
import type { EnvironmentConflict } from "./lib/appPreferences";
import { queryClient } from "./lib/queryClient";
import { scheduleIdleTask } from "./lib/idleTask";
import { pageImports, type RoutePath } from "./lib/routes";
import { useSetWelcomeCompletedMutation } from "./hooks/mutations";

const routeComponents: ReadonlyArray<{
  path: RoutePath;
  Component: React.LazyExoticComponent<ComponentType>;
}> = (Object.keys(pageImports) as RoutePath[]).map((path) => ({
  path,
  Component: lazy(pageImports[path]),
}));

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

function RouteProfiler({ children, pathname }: { children: ReactNode; pathname: string }) {
  const handleRender = useCallback(
    (_id: string, phase: string, actualDuration: number) => {
      if (!import.meta.env.DEV || phase !== "update") {
        return;
      }

      console.debug(`[route-profiler] ${pathname} commit ${actualDuration.toFixed(2)}ms`);
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
        <AppUpdateHost>
          <AppShell />
        </AppUpdateHost>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

function AppShell() {
  const navigate = useNavigate();
  const location = useLocation();
  const setWelcomeCompletedMutation = useSetWelcomeCompletedMutation();
  const [envConflicts, setEnvConflicts] = useState<EnvironmentConflict[]>([]);
  const [bannerDismissed, setBannerDismissed] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [locale, setLocaleState] = useState<Locale>(getLocale());
  const [welcomeOpen, setWelcomeOpen] = useState(false);
  const [welcomeTheme, setWelcomeTheme] = useState<Theme>(getTheme());
  const [installedToolCount, setInstalledToolCount] = useState(0);
  const [profileCount, setProfileCount] = useState(0);
  const lastEnvConflictLoadAtRef = useRef(0);
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;

  // 使用 ref 持有最新 navigate，避免 navigate 引用变化导致全局监听重新绑定。
  const navigateRef = useRef(navigate);
  useEffect(() => {
    navigateRef.current = navigate;
  }, [navigate]);

  useEffect(() => {
    void loadEnvConflicts();
    void loadWelcomeState();
    const cancelPricingSync = scheduleIdleTask(
      () => {
        void invoke("sync_models_dev_pricing", { force: false }).catch((error) => {
          console.debug("Automatic model pricing sync skipped or failed", error);
        });
      },
      { delay: 4_000, timeout: 12_000 },
    );
    const handleFocus = () => void loadEnvConflicts();
    const handleKeyDown = (event: KeyboardEvent) => {
      // 快速路径：非修饰键/非 Escape 直接返回，避免每次按键都走完整流程。
      const hasModifier = event.ctrlKey || event.metaKey;
      if (!hasModifier && event.key !== "Escape") return;

      if (hasModifier) {
        const key = event.key;
        if (key === ",") {
          event.preventDefault();
          navigateRef.current("/settings");
          return;
        }

        // 仅对单字符快捷键做 lowercase 转换
        const lower = key.length === 1 ? key.toLowerCase() : key;
        if (lower === "k") {
          event.preventDefault();
          setCommandPaletteOpen(true);
          return;
        }
        if (lower === "s") {
          event.preventDefault();
          window.dispatchEvent(new CustomEvent("cchub-shortcut-save"));
          return;
        }
        if (lower === "n") {
          event.preventDefault();
          window.dispatchEvent(new CustomEvent("cchub-shortcut-new"));
          return;
        }
        if (lower === "f") {
          const target = event.target as HTMLElement | null;
          if (target?.closest("input, textarea, [contenteditable='true'], .cm-editor")) return;
          event.preventDefault();
          window.dispatchEvent(new CustomEvent("cchub-shortcut-search"));
          return;
        }
        return;
      }

      // Escape：仅在无任何修饰符时分发
      if (!event.altKey && !event.shiftKey) {
        window.dispatchEvent(new CustomEvent("cchub-shortcut-escape"));
      }
    };

    window.addEventListener("focus", handleFocus);
    window.addEventListener("keydown", handleKeyDown);
    const unlistenFailover = listen<{ profile_name: string }>("provider-failover", (event) => {
      showToast("info", `Failover → ${event.payload.profile_name}`);
    });
    return () => {
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("keydown", handleKeyDown);
      cancelPricingSync();
      void unlistenFailover.then((fn) => fn());
    };
    // 该 effect 只需在组件挂载时执行一次，handler 通过 navigateRef 读取最新值。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (envConflicts.length === 0) {
      setBannerDismissed(false);
    }
  }, [envConflicts.length]);

  useEffect(() => {
    setCommandPaletteOpen(false);
  }, [location.pathname]);

  async function loadEnvConflicts() {
    const now = Date.now();
    if (now - lastEnvConflictLoadAtRef.current < 5_000) return;
    lastEnvConflictLoadAtRef.current = now;
    try {
      const conflicts = await invoke<EnvironmentConflict[]>("get_environment_conflicts");
      setEnvConflicts(conflicts);
    } catch (error) {
      console.warn("Failed to load environment conflicts", error);
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
    } catch (error) {
      console.warn("Failed to load welcome state", error);
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
      await setWelcomeCompletedMutation.mutateAsync({ completed: true });
      setWelcomeOpen(false);
    } catch (error) {
      console.error(error);
    }
  }, [setWelcomeCompletedMutation]);

  const highlightVariables = Array.from(new Set(envConflicts.flatMap((item) => item.variables))).slice(0, 4);
  const showConflictBanner = envConflicts.length > 0 && !bannerDismissed && location.pathname !== "/settings";

  return (
    <>
      <ToastContainer />
      <DeepLinkImportHost />
      {welcomeOpen && (
        <Suspense fallback={null}>
          <WelcomeDialog
            open
            locale={locale}
            theme={welcomeTheme}
            installedToolCount={installedToolCount}
            profileCount={profileCount}
            onSelectLocale={handleWelcomeLocaleChange}
            onSelectTheme={handleWelcomeThemeChange}
            onFinish={() => void handleWelcomeFinish()}
          />
        </Suspense>
      )}
      {commandPaletteOpen && (
        <Suspense fallback={null}>
          <CommandPalette
            open={commandPaletteOpen}
            onOpenChange={setCommandPaletteOpen}
            navigate={navigate}
            currentPath={location.pathname}
          />
        </Suspense>
      )}
      <div className="app-layout">
        <Sidebar />
        <div className="main-area">
          <NavigationProgress />
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
                  <div
                    style={{
                      fontSize: 11,
                      color: "var(--text-secondary)",
                      marginTop: 2,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
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
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => setBannerDismissed(true)}
                  title={uiText("关闭", "Dismiss", "閉じる")}
                >
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
