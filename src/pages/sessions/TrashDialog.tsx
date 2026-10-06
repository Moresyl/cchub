import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { RefreshCw, RotateCcw, Trash2 } from "lucide-react";
import ConfirmDialog from "../../components/ConfirmDialog";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../components/ui/dialog";

export interface TrashedSession {
  key: string;
  sessionId: string;
  title?: string | null;
  sourcePath: string;
  deletedAt: string;
  fileCount: number;
  state: string;
  purgeRevision?: string | null;
}

interface PurgeResult {
  purged: string[];
  failed: { key: string; reason: "changed" | "unsafe" | "removeFailed" }[];
}

export default function TrashDialog({
  open,
  onClose,
  onRestored,
  uiText,
}: {
  open: boolean;
  onClose: () => void;
  onRestored: () => void;
  uiText: (zh: string, en: string, ja?: string) => string;
}) {
  const [items, setItems] = useState<TrashedSession[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [restoreErrors, setRestoreErrors] = useState<Record<string, string>>({});
  const [purgeFailures, setPurgeFailures] = useState<{ key: string; title: string }[]>([]);
  const [confirmation, setConfirmation] = useState<TrashedSession[] | null>(null);
  const [purging, setPurging] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const request = useRef(0);
  const restoredKeys = useRef(new Set<string>());
  const restoreInFlight = useRef(false);
  const confirmationTrigger = useRef<HTMLButtonElement | null>(null);
  const refreshButton = useRef<HTMLButtonElement | null>(null);
  const closeButton = useRef<HTMLButtonElement | null>(null);
  const mounted = useRef(true);
  const isOpen = useRef(open);
  isOpen.current = open;
  const busy = Boolean(restoring) || purging;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const load = useCallback(async () => {
    const generation = ++request.current;
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<TrashedSession[]>("list_session_trash");
      if (generation === request.current && isOpen.current)
        setItems(next.filter((item) => !restoredKeys.current.has(item.key)));
    } catch {
      if (generation === request.current && isOpen.current) setError("listFailed");
    } finally {
      if (generation === request.current && isOpen.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (open) void load();
    else setConfirmation(null);
    return () => {
      request.current += 1;
    };
  }, [load, open]);

  const restore = async (item: TrashedSession) => {
    if (restoreInFlight.current) return;
    restoreInFlight.current = true;
    setRestoring(item.key);
    setRestoreErrors((current) => {
      const next = { ...current };
      delete next[item.key];
      return next;
    });
    try {
      await invoke("restore_session_trash", { key: item.key });
      restoredKeys.current.add(item.key);
      if (!mounted.current) return;
      setItems((current) => current.filter((candidate) => candidate.key !== item.key));
      setPurgeFailures((current) => current.filter((candidate) => candidate.key !== item.key));
      onRestored();
    } catch (cause) {
      const collision = /已有不同内容|Existing file was preserved/i.test(String(cause));
      const damaged = /damaged|identity does not match/i.test(String(cause));
      if (mounted.current)
        setRestoreErrors((current) => ({
          ...current,
          [item.key]: collision
            ? uiText(
                "恢复位置已有不同内容，已保留现有文件。",
                "The destination has different content. Existing files were kept.",
                "復元先に異なる内容があります。既存のファイルを保持しました。",
              )
            : damaged
              ? uiText(
                  "恢复副本未通过校验，原始记录已保留。",
                  "The recovery copy failed verification. The recovery entry was kept.",
                  "復元用コピーの検証に失敗しました。記録は保持されています。",
                )
              : uiText(
                  "恢复未完成，请检查配置目录后重试。",
                  "Restore did not complete. Check the configured directory and retry.",
                  "復元が完了しませんでした。設定ディレクトリを確認して再試行してください。",
                ),
        }));
    } finally {
      restoreInFlight.current = false;
      if (mounted.current) setRestoring(null);
    }
  };

  const purge = async () => {
    if (!confirmation || restoreInFlight.current) return;
    const selected = confirmation;
    restoreInFlight.current = true;
    setPurging(true);
    setNotice(null);
    setError(null);
    setPurgeFailures((current) => current.filter((failure) => !selected.some((item) => item.key === failure.key)));
    setRestoreErrors((current) => {
      const next = { ...current };
      selected.forEach((item) => delete next[item.key]);
      return next;
    });
    let refresh = false;
    try {
      const result = await invoke<PurgeResult>("purge_session_trash", {
        targets: selected.map((item) => ({ key: item.key, revision: item.purgeRevision })),
      });
      refresh = true;
      result.purged.forEach((key) => restoredKeys.current.add(key));
      if (!mounted.current) return;
      setItems((current) => current.filter((item) => !restoredKeys.current.has(item.key)));
      setPurgeFailures((current) => [
        ...current,
        ...result.failed.map(({ key }) => ({
          key,
          title:
            selected.find((item) => item.key === key)?.title ||
            selected.find((item) => item.key === key)?.sessionId ||
            key,
        })),
      ]);
      setRestoreErrors((current) => {
        const next = { ...current };
        result.failed.forEach(({ key, reason }) => {
          next[key] =
            reason === "changed"
              ? uiText(
                  "记录已变化，已停止清理。刷新后重新确认删除。",
                  "This entry changed. Cleanup stopped. Refresh and review it again.",
                  "記録が変更されたため削除を停止しました。更新して再確認してください。",
                )
              : reason === "unsafe"
                ? uiText(
                    "发现无法安全删除的文件，已停止清理。",
                    "Some files cannot be safely removed. Cleanup stopped.",
                    "安全に削除できないファイルがあるため削除を停止しました。",
                  )
                : uiText(
                    "部分文件未能清理，请刷新后重试。",
                    "Some files could not be removed. Refresh and try again.",
                    "一部のファイルを削除できませんでした。更新して再試行してください。",
                  );
        });
        return next;
      });
      if (result.purged.length)
        setNotice(
          uiText(
            `已彻底删除 ${result.purged.length} 条会话`,
            `Permanently deleted ${result.purged.length} sessions`,
            `${result.purged.length} 件の会話を完全に削除しました`,
          ),
        );
    } catch {
      if (mounted.current)
        setError(
          uiText(
            "清理未完成，请刷新列表后重试。",
            "Cleanup did not complete. Refresh the list and try again.",
            "削除が完了しませんでした。一覧を更新して再試行してください。",
          ),
        );
    } finally {
      restoreInFlight.current = false;
      if (mounted.current) {
        setPurging(false);
        setConfirmation(null);
        // Refresh revisions after partial failures; never broaden the confirmed set.
        if (refresh && isOpen.current) void load();
      }
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={(next) => !next && !busy && !confirmation && onClose()}>
        <DialogContent hideClose fullscreenOnMobile={false} className="max-w-[560px]">
          <DialogHeader className="pr-5">
            <div className="min-w-0">
              <DialogTitle>{uiText("最近删除", "Recently deleted", "最近削除した会話")}</DialogTitle>
              <DialogDescription>
                {uiText(
                  "可恢复 Codex 会话，或确认后彻底删除。恢复不会覆盖已有内容，记录会保留到你主动清理。",
                  "Restore Codex sessions or permanently delete them after confirmation. Restore preserves existing content. Entries stay until you choose to remove them.",
                  "Codex 会話を復元、または確認後に完全削除できます。既存の内容は上書きしません。記録は手動で削除するまで保持されます。",
                )}
              </DialogDescription>
            </div>
          </DialogHeader>
          <DialogBody aria-busy={loading || busy} className="space-y-3">
            {notice && (
              <p role="status" className="text-[12px] text-muted-foreground">
                {notice}
              </p>
            )}
            {error && (
              <div role="alert" className="space-y-2 break-words text-[12px] text-[var(--danger)]">
                <p>
                  {error === "listFailed"
                    ? uiText(
                        "无法读取最近删除列表，请重试。",
                        "Could not read recently deleted sessions. Please retry.",
                        "最近削除した会話を読み込めませんでした。再試行してください。",
                      )
                    : error}
                </p>
                <Button variant="secondary" disabled={loading || busy} onClick={() => void load()}>
                  {uiText("重试", "Retry", "再試行")}
                </Button>
              </div>
            )}
            {loading && !items.length && (
              <p role="status" className="text-[12px] text-muted-foreground">
                {uiText("加载最近删除的会话…", "Loading deleted sessions…", "削除した会話を読み込み中…")}
              </p>
            )}
            {!loading && !error && !items.length && !purgeFailures.length && (
              <p className="py-6 text-center text-[12px] text-muted-foreground">
                {uiText("暂无可恢复的会话", "No sessions to restore", "復元できる会話はありません")}
              </p>
            )}
            {items.map((item) => (
              <Card key={item.key} className="p-3">
                <div className="flex flex-col gap-3 sm:flex-row sm:items-start">
                  <div className="min-w-0 flex-1">
                    <p className="break-words text-[12px] font-medium">{item.title || item.sessionId}</p>
                    <p title={item.sourcePath} className="mt-1 truncate text-[11px] text-muted-foreground">
                      {item.sourcePath}
                    </p>
                    <p className="mt-2 text-[11px] text-muted-foreground">
                      {new Date(item.deletedAt).toLocaleString()} ·{" "}
                      {uiText(`${item.fileCount} 个文件`, `${item.fileCount} files`, `${item.fileCount} ファイル`)}
                    </p>
                  </div>
                  <div className="flex shrink-0 justify-end gap-2">
                    <Button variant="secondary" disabled={busy || loading} onClick={() => void restore(item)}>
                      <RotateCcw size={14} />
                      {restoring === item.key
                        ? uiText("恢复中…", "Restoring…", "復元中…")
                        : uiText("恢复", "Restore", "復元")}
                    </Button>
                    <Button
                      variant="ghost"
                      disabled={busy || loading || !item.purgeRevision}
                      aria-label={uiText(
                        `彻底删除：${item.title || item.sessionId}`,
                        `Permanently delete: ${item.title || item.sessionId}`,
                        `完全削除：${item.title || item.sessionId}`,
                      )}
                      title={
                        item.purgeRevision
                          ? undefined
                          : uiText(
                              "记录需重新核对，暂时无法清理",
                              "This entry needs verification before cleanup",
                              "削除前に記録の確認が必要です",
                            )
                      }
                      onClick={(event) => {
                        confirmationTrigger.current = event.currentTarget;
                        setConfirmation([item]);
                      }}
                    >
                      <Trash2 size={14} aria-hidden="true" />
                      {uiText("彻底删除", "Delete forever", "完全削除")}
                    </Button>
                  </div>
                </div>
                {!item.purgeRevision && (
                  <p className="mt-2 text-[11px] text-muted-foreground">
                    {uiText(
                      "该记录需重新核对，暂时无法彻底删除。",
                      "This entry needs verification before permanent deletion.",
                      "この記録は完全削除前に確認が必要です。",
                    )}
                  </p>
                )}
                {restoreErrors[item.key] && (
                  <p role="alert" className="mt-2 break-words text-[12px] text-[var(--danger)]">
                    {restoreErrors[item.key]}
                  </p>
                )}
              </Card>
            ))}
            {purgeFailures
              .filter((failure) => !items.some((item) => item.key === failure.key))
              .map((failure) => (
                <Card key={failure.key} className="space-y-2 p-3">
                  <p className="break-words text-[12px] font-medium">{failure.title}</p>
                  <p role="alert" className="break-words text-[12px] text-[var(--danger)]">
                    {restoreErrors[failure.key]}
                  </p>
                </Card>
              ))}
          </DialogBody>
          <DialogFooter className="flex-wrap">
            <Button
              variant="ghost"
              disabled={loading || busy || !items.length || items.some((item) => !item.purgeRevision)}
              onClick={(event) => {
                confirmationTrigger.current = event.currentTarget;
                setConfirmation([...items]);
              }}
            >
              <Trash2 size={14} aria-hidden="true" />
              {uiText("清空最近删除", "Empty recently deleted", "最近削除した会話を空にする")}
            </Button>
            <Button ref={refreshButton} variant="ghost" disabled={loading || busy} onClick={() => void load()}>
              <RefreshCw size={14} />
              {uiText("刷新", "Refresh", "更新")}
            </Button>
            <Button ref={closeButton} variant="secondary" disabled={busy} onClick={onClose}>
              {uiText("关闭", "Close", "閉じる")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        isOpen={open && Boolean(confirmation)}
        busy={purging}
        title={uiText("彻底删除会话？", "Permanently delete sessions?", "会話を完全に削除しますか？")}
        message={
          confirmation
            ? uiText(
                `将永久移除这 ${confirmation.length} 条会话的恢复副本，此操作无法撤销。\n${confirmation
                  .slice(0, 5)
                  .map((item) => item.title || item.sessionId)
                  .join("\n")}${confirmation.length > 5 ? `\n… 共 ${confirmation.length} 条` : ""}`,
                `Permanently remove recovery copies of these ${confirmation.length} sessions. This cannot be undone.\n${confirmation
                  .slice(0, 5)
                  .map((item) => item.title || item.sessionId)
                  .join("\n")}${confirmation.length > 5 ? `\n… ${confirmation.length} sessions total` : ""}`,
                `この ${confirmation.length} 件の会話の復元用コピーを完全に削除します。元に戻せません。\n${confirmation
                  .slice(0, 5)
                  .map((item) => item.title || item.sessionId)
                  .join("\n")}${confirmation.length > 5 ? `\n… 合計 ${confirmation.length} 件` : ""}`,
              )
            : ""
        }
        confirmText={
          purging ? uiText("删除中…", "Deleting…", "削除中…") : uiText("彻底删除", "Delete forever", "完全削除")
        }
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        onConfirm={() => void purge()}
        onCancel={() => setConfirmation(null)}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          if (isOpen.current) {
            const trigger = confirmationTrigger.current;
            if (trigger?.isConnected && !trigger.disabled) trigger.focus();
            else if (refreshButton.current && !refreshButton.current.disabled) refreshButton.current.focus();
            else closeButton.current?.focus();
          }
        }}
      />
    </>
  );
}
