import { memo, useCallback, useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getLocale } from "../../lib/i18n";
import { Button } from "../ui/button";

export type DesktopPlatform = "windows" | "macos" | "linux" | "web";
export type WindowAction = "minimize" | "maximize" | "close";

export function detectDesktopPlatform(userAgent: string): DesktopPlatform {
  if (/Macintosh|Mac OS X/i.test(userAgent)) return "macos";
  if (/Windows/i.test(userAgent)) return "windows";
  if (/Linux/i.test(userAgent)) return "linux";
  return "web";
}

export function isTauriWindow(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

export async function runWindowAction(action: WindowAction): Promise<void> {
  if (!isTauriWindow()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const currentWindow = getCurrentWindow();
  if (action === "minimize") await currentWindow.minimize();
  if (action === "maximize") await currentWindow.toggleMaximize();
  if (action === "close") await currentWindow.close();
}

interface WindowControlsProps {
  platform: DesktopPlatform;
}

function WindowControlsComponent({ platform }: WindowControlsProps) {
  const [maximized, setMaximized] = useState(false);
  const locale = getLocale();
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);

  useEffect(() => {
    if (platform === "macos" || platform === "web" || !isTauriWindow()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const currentWindow = getCurrentWindow();
        const syncMaximized = async () => {
          const next = await currentWindow.isMaximized();
          if (!disposed) setMaximized(next);
        };
        await syncMaximized();
        unlisten = await currentWindow.onResized(() => void syncMaximized());
      })
      .catch((error) => console.warn("Failed to observe window state", error));

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [platform]);

  const handleAction = useCallback(async (action: WindowAction) => {
    try {
      await runWindowAction(action);
      if (action === "maximize" && isTauriWindow()) {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        setMaximized(await getCurrentWindow().isMaximized());
      }
    } catch (error) {
      console.warn(`Window action failed: ${action}`, error);
    }
  }, []);

  if (platform === "macos" || platform === "web") return null;

  return (
    <div className="window-controls" aria-label={text("窗口控制", "Window controls", "ウィンドウ操作")}>
      <Button
        variant="ghost"
        className="window-control-button"
        onClick={() => void handleAction("minimize")}
        aria-label={text("最小化", "Minimize", "最小化")}
        title={text("最小化", "Minimize", "最小化")}
      >
        <Minus size={15} strokeWidth={1.5} aria-hidden="true" />
      </Button>
      <Button
        variant="ghost"
        className="window-control-button"
        onClick={() => void handleAction("maximize")}
        aria-label={maximized ? text("还原", "Restore", "元に戻す") : text("最大化", "Maximize", "最大化")}
        title={maximized ? text("还原", "Restore", "元に戻す") : text("最大化", "Maximize", "最大化")}
      >
        {maximized ? (
          <Copy size={13} strokeWidth={1.5} aria-hidden="true" />
        ) : (
          <Square size={12} strokeWidth={1.5} aria-hidden="true" />
        )}
      </Button>
      <Button
        variant="ghost"
        className="window-control-button window-control-close"
        onClick={() => void handleAction("close")}
        aria-label={text("关闭", "Close", "閉じる")}
        title={text("关闭", "Close", "閉じる")}
      >
        <X size={15} strokeWidth={1.5} aria-hidden="true" />
      </Button>
    </div>
  );
}

export default memo(WindowControlsComponent);
