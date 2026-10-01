import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { RefreshCw, RotateCcw } from "lucide-react";
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
  const request = useRef(0);
  const restoredKeys = useRef(new Set<string>());
  const restoreInFlight = useRef(false);
  const isOpen = useRef(open);
  isOpen.current = open;
  const load = useCallback(async () => {
    const generation = ++request.current;
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<TrashedSession[]>("list_session_trash");
      if (generation === request.current && isOpen.current)
        setItems(next.filter((item) => !restoredKeys.current.has(item.key)));
    } catch (cause) {
      if (generation === request.current && isOpen.current) setError(String(cause));
    } finally {
      if (generation === request.current && isOpen.current) setLoading(false);
    }
  }, []);
  useEffect(() => {
    if (open) void load();
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
      setItems((current) => current.filter((candidate) => candidate.key !== item.key));
      onRestored();
    } catch (cause) {
      setRestoreErrors((current) => ({ ...current, [item.key]: String(cause) }));
    } finally {
      restoreInFlight.current = false;
      setRestoring(null);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      <DialogContent hideClose fullscreenOnMobile={false} className="max-w-[560px]">
        <DialogHeader className="pr-5">
          <div className="min-w-0">
            <DialogTitle>{uiText("最近删除", "Recently deleted", "最近削除した会話")}</DialogTitle>
            <DialogDescription>
              {uiText(
                "Codex 会话文件可在这里恢复。已有文件会保留，不会覆盖你的新内容。",
                "Restore deleted Codex session files. Existing files and newer content are kept.",
                "削除した Codex 会話ファイルを復元できます。既存ファイルや新しい内容は上書きしません。",
              )}
            </DialogDescription>
          </div>
        </DialogHeader>
        <DialogBody aria-busy={loading} className="space-y-3">
          {error && (
            <div role="alert" className="space-y-2 break-words text-[12px] text-[var(--danger)]">
              <p>{error}</p>
              <Button variant="secondary" onClick={() => void load()}>
                {uiText("重试", "Retry", "再試行")}
              </Button>
            </div>
          )}
          {loading && !items.length && (
            <p role="status" className="text-[12px] text-muted-foreground">
              {uiText("加载最近删除的会话…", "Loading deleted sessions…", "削除した会話を読み込み中…")}
            </p>
          )}
          {!loading && !error && !items.length && (
            <p className="py-6 text-center text-[12px] text-muted-foreground">
              {uiText("暂无可恢复的会话", "No sessions to restore", "復元できる会話はありません")}
            </p>
          )}
          {items.map((item) => (
            <Card key={item.key} className="p-3">
              <div className="flex items-start gap-3">
                <div className="min-w-0 flex-1">
                  <p className="break-words text-[12px] font-[510]">{item.title || item.sessionId}</p>
                  <p title={item.sourcePath} className="mt-1 line-clamp-2 break-all text-[11px] text-muted-foreground">
                    {item.sourcePath}
                  </p>
                  <p className="mt-2 text-[11px] text-muted-foreground">
                    {new Date(item.deletedAt).toLocaleString()} ·{" "}
                    {uiText(`${item.fileCount} 个文件`, `${item.fileCount} files`, `${item.fileCount} ファイル`)}
                  </p>
                </div>
                <Button variant="secondary" disabled={Boolean(restoring)} onClick={() => void restore(item)}>
                  <RotateCcw size={14} />
                  {restoring === item.key
                    ? uiText("恢复中…", "Restoring…", "復元中…")
                    : uiText("恢复", "Restore", "復元")}
                </Button>
              </div>
              {restoreErrors[item.key] && (
                <p role="alert" className="mt-2 break-words text-[12px] text-[var(--danger)]">
                  {restoreErrors[item.key]}
                </p>
              )}
            </Card>
          ))}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" disabled={loading || Boolean(restoring)} onClick={() => void load()}>
            <RefreshCw size={14} />
            {uiText("刷新", "Refresh", "更新")}
          </Button>
          <Button variant="secondary" onClick={onClose}>
            {uiText("关闭", "Close", "閉じる")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
