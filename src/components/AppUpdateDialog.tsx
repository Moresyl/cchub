import { AlertCircle, CheckCircle, Download, RefreshCw, X } from "lucide-react";
import { useEffect, useRef } from "react";
import type { AppUpdateResult } from "../lib/appUpdater";
import { localizedReleaseNotes } from "../lib/releaseNotes";
import { usePreferences } from "../stores/preferences";
import ReleaseNotes from "./ReleaseNotes";
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
  const locale = usePreferences((state) => state.locale);
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const hasUpdate = update?.update_available ?? false;
  const notes = localizedReleaseNotes(update?.body, locale);
  const notesRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (notesRef.current) notesRef.current.scrollTop = 0;
  }, [notes, locale]);
  const primaryLabel = installing
    ? text("正在更新…", "Updating…", "更新中…")
    : update?.can_install
      ? text("一键更新并重启", "Update and restart", "更新して再起動")
      : text("打开 GitHub 下载", "Open GitHub downloads", "GitHub のダウンロードを開く");

  const title = checking
    ? text("正在检查更新", "Checking for updates", "更新を確認中")
    : error && !hasUpdate
      ? text("未能检查更新", "Could not check for updates", "更新を確認できませんでした")
      : update?.disabled_by_env
        ? text("自动更新已关闭", "Automatic updates are disabled", "自動更新は無効です")
        : update?.not_configured
          ? text("更新渠道暂不可用", "Update channel is unavailable", "更新チャンネルを利用できません")
          : hasUpdate
            ? text(
                `发现新版本 v${update?.latest_version ?? ""}`,
                `Version ${update?.latest_version ?? ""} is available`,
                `新しいバージョン v${update?.latest_version ?? ""} があります`,
              )
            : update
              ? text("当前已是最新版本", "CCHub is up to date", "CCHub は最新です")
              : text("检查更新", "Check for updates", "更新を確認");
  const statusIcon = checking ? (
    <RefreshCw size={17} className="spin" aria-hidden="true" />
  ) : error ? (
    <AlertCircle size={17} aria-hidden="true" />
  ) : hasUpdate ? (
    <Download size={17} aria-hidden="true" />
  ) : update && !update.disabled_by_env && !update.not_configured ? (
    <CheckCircle size={17} aria-hidden="true" />
  ) : (
    <RefreshCw size={17} aria-hidden="true" />
  );

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && !installing && onClose()}>
      <DialogContent hideClose className="max-w-[640px]">
        <DialogHeader>
          <div
            className="grid size-9 shrink-0 place-items-center rounded-[7px]"
            style={{
              background: error ? "var(--danger-subtle)" : "var(--accent-subtle)",
              color: error ? "var(--danger)" : "var(--accent)",
            }}
          >
            {statusIcon}
          </div>
          <div className="min-w-0 flex-1">
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>
              {update?.current_version
                ? `${text("当前版本", "Current version", "現在のバージョン")}: v${update.current_version}`
                : text(
                    "检查发布渠道中的最新稳定版本",
                    "Check the release channel for the latest stable version",
                    "更新チャンネルの最新安定版を確認します",
                  )}
            </DialogDescription>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            disabled={installing}
            aria-label={text("关闭", "Close", "閉じる")}
            title={text("关闭", "Close", "閉じる")}
          >
            <X size={14} aria-hidden="true" />
          </Button>
        </DialogHeader>

        <DialogBody className="flex flex-col gap-4">
          {checking && <div className="spinner mx-auto my-6 size-6" />}

          {!checking && hasUpdate && (
            <section className="flex min-h-0 flex-1 flex-col">
              <h4 className="mb-2 text-xs font-semibold">{text("本次更新", "What's new", "更新内容")}</h4>
              <div
                ref={notesRef}
                className="app-release-notes min-h-0 rounded-md border border-border bg-[var(--bg-input)] px-3.5 py-3"
              >
                {notes ? (
                  <ReleaseNotes content={notes} />
                ) : (
                  <p>
                    {text(
                      "此版本未提供更新说明。",
                      "No release notes were provided.",
                      "更新内容が提供されていません。",
                    )}
                  </p>
                )}
              </div>
            </section>
          )}

          {installing && update?.can_install && (
            <section className="shrink-0">
              <div
                role="progressbar"
                aria-label={text("更新下载进度", "Update download progress", "更新のダウンロード進捗")}
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
                {installProgress === null
                  ? text("正在准备下载…", "Preparing download…", "ダウンロードを準備中…")
                  : `${installProgress}%`}
              </p>
            </section>
          )}

          {!checking && error && (
            <div
              className="flex shrink-0 gap-2 rounded-md border border-[var(--danger)]/20 bg-[var(--danger-subtle)] px-3 py-2.5 text-xs text-[var(--danger)]"
              role="alert"
            >
              <AlertCircle size={15} className="shrink-0" aria-hidden="true" />
              <span>{error}</span>
            </div>
          )}
        </DialogBody>

        <DialogFooter>
          <Button variant="secondary" onClick={onCheck} disabled={checking || installing}>
            <RefreshCw size={14} className={checking ? "spin" : ""} aria-hidden="true" />
            {text("重新检查", "Check again", "再確認")}
          </Button>
          {hasUpdate && (
            <Button onClick={onInstall} disabled={installing || checking}>
              <Download size={14} aria-hidden="true" />
              {primaryLabel}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
