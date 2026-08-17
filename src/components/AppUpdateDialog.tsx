import { useEffect, useRef } from "react";
import { AlertCircle, CheckCircle, Download, RefreshCw, X } from "lucide-react";
import type { AppUpdateResult } from "../lib/appUpdater";
import { getLocale } from "../lib/i18n";

interface AppUpdateDialogProps {
  isOpen: boolean;
  update: AppUpdateResult | null;
  checking: boolean;
  installing: boolean;
  installProgress: number | null;
  error: string | null;
  onClose: () => void;
  onCheck: () => void;
  onInstall: () => void;
}

export default function AppUpdateDialog({
  isOpen,
  update,
  checking,
  installing,
  installProgress,
  error,
  onClose,
  onCheck,
  onInstall,
}: AppUpdateDialogProps) {
  const zh = getLocale() === "zh";
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!isOpen) return;
    closeButtonRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !installing) onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [installing, isOpen, onClose]);

  if (!isOpen) return null;

  const hasUpdate = update?.update_available ?? false;
  const primaryLabel = installing
    ? zh
      ? "正在下载并安装…"
      : "Downloading and installing…"
    : update?.can_install
      ? zh
        ? "一键更新并重启"
        : "Update and restart"
      : zh
        ? "打开 GitHub 下载"
        : "Open GitHub downloads";

  return (
    <div className="confirm-overlay" onClick={installing ? undefined : onClose} role="presentation">
      <div
        className="confirm-dialog animate-in"
        role="dialog"
        aria-modal="true"
        aria-labelledby="app-update-title"
        onClick={(event) => event.stopPropagation()}
        style={{ width: "min(640px, calc(100vw - 40px))", maxWidth: 640 }}
      >
        <div style={{ display: "flex", alignItems: "flex-start", gap: 14 }}>
          <div
            style={{
              width: 42,
              height: 42,
              borderRadius: 10,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: hasUpdate ? "var(--accent-subtle)" : "var(--success-subtle)",
              flexShrink: 0,
            }}
          >
            {hasUpdate ? (
              <Download size={20} style={{ color: "var(--accent)" }} />
            ) : (
              <CheckCircle size={20} style={{ color: "var(--success)" }} />
            )}
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: "flex", justifyContent: "space-between", gap: 12 }}>
              <div>
                <h3 id="app-update-title" style={{ fontSize: 16, fontWeight: 650 }}>
                  {checking
                    ? zh
                      ? "正在检查更新"
                      : "Checking for updates"
                    : hasUpdate
                      ? zh
                        ? `发现新版本 v${update?.latest_version ?? ""}`
                        : `Version ${update?.latest_version ?? ""} is available`
                      : zh
                        ? "当前已是最新版本"
                        : "CCHub is up to date"}
                </h3>
                {update?.current_version && (
                  <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
                    {zh ? "当前版本" : "Current version"}: v{update.current_version}
                  </p>
                )}
              </div>
              <button
                ref={closeButtonRef}
                className="btn btn-ghost btn-icon-sm"
                onClick={onClose}
                disabled={installing}
                aria-label={zh ? "关闭" : "Close"}
                title={zh ? "关闭" : "Close"}
              >
                <X size={16} />
              </button>
            </div>

            {checking && <div className="spinner" style={{ width: 22, height: 22, marginTop: 24 }} />}

            {!checking && hasUpdate && (
              <div style={{ marginTop: 18 }}>
                <p style={{ fontSize: 12, fontWeight: 600, marginBottom: 8 }}>{zh ? "本次更新" : "What's new"}</p>
                <div
                  style={{
                    maxHeight: 280,
                    overflowY: "auto",
                    padding: "12px 14px",
                    borderRadius: 8,
                    background: "var(--bg-input)",
                    border: "1px solid var(--border-default)",
                    color: "var(--text-secondary)",
                    fontSize: 12,
                    lineHeight: 1.65,
                    whiteSpace: "pre-wrap",
                  }}
                >
                  {update?.body?.trim() || (zh ? "此版本未提供更新说明。" : "No release notes were provided.")}
                </div>
              </div>
            )}

            {installing && update?.can_install && (
              <div style={{ marginTop: 16 }}>
                <div
                  role="progressbar"
                  aria-label={zh ? "更新下载进度" : "Update download progress"}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={installProgress ?? undefined}
                  style={{ height: 6, overflow: "hidden", borderRadius: 999, background: "var(--bg-badge)" }}
                >
                  <div
                    style={{
                      width: `${installProgress ?? 12}%`,
                      height: "100%",
                      borderRadius: 999,
                      background: "var(--accent)",
                      transition: "width 0.16s ease",
                    }}
                  />
                </div>
                <p style={{ marginTop: 6, fontSize: 11, color: "var(--text-muted)", textAlign: "right" }}>
                  {installProgress === null ? (zh ? "正在准备下载…" : "Preparing download…") : `${installProgress}%`}
                </p>
              </div>
            )}

            {!checking && error && (
              <div style={{ display: "flex", gap: 8, color: "var(--danger)", marginTop: 16, fontSize: 12 }}>
                <AlertCircle size={15} style={{ flexShrink: 0 }} />
                <span>{error}</span>
              </div>
            )}
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
          <button className="btn btn-secondary btn-sm" onClick={onCheck} disabled={checking || installing}>
            <RefreshCw size={14} className={checking ? "spin" : ""} />
            {zh ? "重新检查" : "Check again"}
          </button>
          {hasUpdate && (
            <button className="btn btn-primary btn-sm" onClick={onInstall} disabled={installing}>
              <Download size={14} />
              {primaryLabel}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
