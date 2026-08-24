import { AlertCircle, CheckCircle, Download, RefreshCw, X } from "lucide-react";
import type { AppUpdateResult } from "../lib/appUpdater";
import { getLocale } from "../lib/i18n";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

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

  const title = checking
    ? zh
      ? "正在检查更新"
      : "Checking for updates"
    : hasUpdate
      ? zh
        ? `发现新版本 v${update?.latest_version ?? ""}`
        : `Version ${update?.latest_version ?? ""} is available`
      : zh
        ? "当前已是最新版本"
        : "CCHub is up to date";

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && !installing && onClose()}>
      <DialogContent hideClose className="max-w-[640px]">
        <DialogHeader>
          <div
            className="grid size-9 shrink-0 place-items-center rounded-[7px]"
            style={{
              background: hasUpdate ? "var(--accent-subtle)" : "var(--success-subtle)",
              color: hasUpdate ? "var(--accent)" : "var(--success)",
            }}
          >
            {hasUpdate ? <Download size={17} aria-hidden="true" /> : <CheckCircle size={17} aria-hidden="true" />}
          </div>
          <div className="min-w-0 flex-1">
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>
              {update?.current_version
                ? `${zh ? "当前版本" : "Current version"}: v${update.current_version}`
                : zh
                  ? "检查发布渠道中的最新稳定版本"
                  : "Check the release channel for the latest stable version"}
            </DialogDescription>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            disabled={installing}
            aria-label={zh ? "关闭" : "Close"}
            title={zh ? "关闭" : "Close"}
          >
            <X size={14} aria-hidden="true" />
          </Button>
        </DialogHeader>

        <DialogBody className="space-y-4">
          {checking && <div className="spinner mx-auto my-6 size-6" />}

          {!checking && hasUpdate && (
            <section>
              <h4 className="mb-2 text-xs font-semibold">{zh ? "本次更新" : "What's new"}</h4>
              <div className="max-h-[280px] overflow-y-auto whitespace-pre-wrap rounded-md border border-border bg-[var(--bg-input)] px-3.5 py-3 text-xs leading-relaxed text-muted-foreground">
                {update?.body?.trim() || (zh ? "此版本未提供更新说明。" : "No release notes were provided.")}
              </div>
            </section>
          )}

          {installing && update?.can_install && (
            <section>
              <div
                role="progressbar"
                aria-label={zh ? "更新下载进度" : "Update download progress"}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={installProgress ?? undefined}
                className="h-1.5 overflow-hidden rounded-full bg-[var(--bg-badge)]"
              >
                <div
                  className="h-full rounded-full bg-primary transition-[width] duration-150"
                  style={{ width: `${installProgress ?? 12}%` }}
                />
              </div>
              <p className="mt-1.5 text-right text-[11px] text-muted-foreground">
                {installProgress === null ? (zh ? "正在准备下载…" : "Preparing download…") : `${installProgress}%`}
              </p>
            </section>
          )}

          {!checking && error && (
            <div
              className="flex gap-2 rounded-md border border-[var(--danger)]/20 bg-[var(--danger-subtle)] px-3 py-2.5 text-xs text-[var(--danger)]"
              role="alert"
            >
              <AlertCircle size={15} className="shrink-0" aria-hidden="true" />
              <span>{error}</span>
            </div>
          )}
        </DialogBody>

        <DialogFooter>
          <Button variant="secondary" size="sm" onClick={onCheck} disabled={checking || installing}>
            <RefreshCw size={14} className={checking ? "spin" : ""} aria-hidden="true" />
            {zh ? "重新检查" : "Check again"}
          </Button>
          {hasUpdate && (
            <Button size="sm" onClick={onInstall} disabled={installing}>
              <Download size={14} aria-hidden="true" />
              {primaryLabel}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
