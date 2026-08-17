import {
  createContext,
  lazy,
  Suspense,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { checkAppUpdate, installAppUpdate, type AppUpdateHandle, type AppUpdateResult } from "../lib/appUpdater";
import { scheduleIdleTask } from "../lib/idleTask";
import { showToast } from "./Toast";
import { getLocale } from "../lib/i18n";

const AppUpdateDialog = lazy(() => import("./AppUpdateDialog"));
const APP_UPDATE_EVENT = "cchub:show-app-update";

interface AppUpdateContextValue {
  updateAvailable: boolean;
  latestVersion: string | null;
  openUpdateDialog: () => void;
}

const AppUpdateContext = createContext<AppUpdateContextValue | null>(null);

export function requestAppUpdateDialog() {
  window.dispatchEvent(new CustomEvent(APP_UPDATE_EVENT));
}

export function useAppUpdate() {
  const value = useContext(AppUpdateContext);
  if (!value) throw new Error("useAppUpdate must be used within AppUpdateHost");
  return value;
}

export default function AppUpdateHost({ children }: { children: ReactNode }) {
  const [update, setUpdate] = useState<AppUpdateResult | null>(null);
  const [handle, setHandle] = useState<AppUpdateHandle | null>(null);
  const [open, setOpen] = useState(false);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const checkingRef = useRef(false);

  const checkForUpdate = useCallback(async (showDialog: boolean) => {
    if (showDialog) setOpen(true);
    if (checkingRef.current) return;
    checkingRef.current = true;
    setChecking(true);
    setError(null);
    try {
      const next = await checkAppUpdate();
      setUpdate(next.result);
      setHandle(next.handle);
      if (!showDialog && next.result.update_available && next.result.latest_version) {
        showToast(
          "info",
          getLocale() === "zh"
            ? `发现新版本 v${next.result.latest_version}`
            : `Version ${next.result.latest_version} is available`,
          8000,
        );
      }
    } catch (nextError) {
      setError(String(nextError));
      setHandle(null);
    } finally {
      checkingRef.current = false;
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    const cancelCheck = scheduleIdleTask(() => void checkForUpdate(false), { delay: 2500, timeout: 10_000 });
    const handleShow = () => void checkForUpdate(true);
    window.addEventListener(APP_UPDATE_EVENT, handleShow);
    return () => {
      cancelCheck();
      window.removeEventListener(APP_UPDATE_EVENT, handleShow);
    };
  }, [checkForUpdate]);

  const openUpdateDialog = useCallback(() => {
    setOpen(true);
    if (!update && !checkingRef.current) void checkForUpdate(true);
  }, [checkForUpdate, update]);

  const install = useCallback(async () => {
    if (!handle) return;
    setInstalling(true);
    setError(null);
    try {
      await installAppUpdate(handle);
    } catch (nextError) {
      setError(String(nextError));
    } finally {
      setInstalling(false);
    }
  }, [handle]);

  return (
    <AppUpdateContext.Provider
      value={{
        updateAvailable: update?.update_available ?? false,
        latestVersion: update?.latest_version ?? null,
        openUpdateDialog,
      }}
    >
      {children}
      {open && (
        <Suspense fallback={null}>
          <AppUpdateDialog
            isOpen={open}
            update={update}
            checking={checking}
            installing={installing}
            error={error}
            onClose={() => setOpen(false)}
            onCheck={() => void checkForUpdate(true)}
            onInstall={() => void install()}
          />
        </Suspense>
      )}
    </AppUpdateContext.Provider>
  );
}
