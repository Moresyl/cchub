import { useState } from "react";
import { Database, RotateCcw } from "lucide-react";
import { compatApi, type SessionSyncResult } from "../lib/api/compat";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import { useAppDialog } from "./AppDialogProvider";
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

type Action = "sync" | "rebuild";

function resultMessage(result: SessionSyncResult, locale: string, action: Action) {
  const prefix = result.errors.length
    ? locale === "zh"
      ? result.imported || result.updated
        ? "部分会话用量已同步"
        : "用量同步未完成"
      : result.imported || result.updated
        ? "Session usage partially synced"
        : "Usage sync incomplete"
    : action === "rebuild"
      ? locale === "zh"
        ? "Codex 用量已重建"
        : "Codex usage rebuilt"
      : locale === "zh"
        ? "会话用量已同步"
        : "Session usage synced";
  const parts = [`${result.imported} ${locale === "zh" ? "条新增" : "imported"}`];
  if (result.updated) parts.push(`${result.updated} ${locale === "zh" ? "条更新" : "updated"}`);
  parts.push(`${result.suspectedDuplicates} ${locale === "zh" ? "条重复" : "duplicates"}`);
  if (result.deferredFiles)
    parts.push(`${result.deferredFiles} ${locale === "zh" ? "个来源等待下次同步" : "sources pending retry"}`);
  return `${prefix}: ${parts.join(locale === "zh" ? "，" : ", ")}`;
}

export default function SessionUsageActions() {
  const locale = getLocale();
  const appDialog = useAppDialog();
  const [busy, setBusy] = useState<Action | null>(null);
  const [lastResult, setLastResult] = useState<{ result: SessionSyncResult; action: Action } | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);

  async function run(action: Action) {
    if (action === "rebuild") {
      const confirmed = await appDialog.confirm({
        title: locale === "zh" ? "重建 Codex 用量" : "Rebuild Codex usage",
        message:
          locale === "zh"
            ? "现有 Codex 会话用量记录会先被清理，再从本地会话重新导入。"
            : "Existing Codex usage records will be cleared, then re-imported from local sessions.",
        confirmText: locale === "zh" ? "继续重建" : "Rebuild",
        cancelText: locale === "zh" ? "取消" : "Cancel",
        tone: "warning",
      });
      if (!confirmed) return;
    }
    setBusy(action);
    try {
      const result = action === "rebuild" ? await compatApi.rebuildCodexUsage() : await compatApi.syncSessionUsage();
      setLastResult({ result, action });
      if (result.errors.length) {
        setDetailsOpen(true);
      } else {
        showToast(result.deferredFiles ? "info" : "success", resultMessage(result, locale, action));
      }
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setBusy(null);
    }
  }

  return (
    <>
      <Button
        type="button"
        variant="secondary"
        onClick={() => void run("sync")}
        disabled={busy !== null}
        aria-busy={busy === "sync"}
      >
        <Database size={14} className={busy === "sync" ? "spin" : undefined} />
        {locale === "zh" ? "同步用量" : "Sync usage"}
      </Button>
      <Button
        type="button"
        variant="secondary"
        onClick={() => void run("rebuild")}
        disabled={busy !== null}
        aria-busy={busy === "rebuild"}
      >
        <RotateCcw size={14} className={busy === "rebuild" ? "spin" : undefined} />
        {locale === "zh" ? "重建 Codex" : "Rebuild Codex"}
      </Button>
      {lastResult && (
        <Button type="button" variant="ghost" onClick={() => setDetailsOpen(true)}>
          {locale === "zh" ? "同步详情" : "Sync details"}
        </Button>
      )}
      <Dialog open={detailsOpen} onOpenChange={setDetailsOpen}>
        <DialogContent>
          <DialogHeader>
            <div>
              <DialogTitle>{locale === "zh" ? "用量同步结果" : "Usage sync result"}</DialogTitle>
              <DialogDescription>
                {lastResult?.result.errors.length
                  ? locale === "zh"
                    ? "部分来源未能同步，请查看原因后重试。"
                    : "Some sources could not sync. Review the errors and retry."
                  : locale === "zh"
                    ? "已导入的记录会保留，重复记录不会再次计费。"
                    : "Imported records are retained; duplicates are not billed again."}
              </DialogDescription>
            </div>
          </DialogHeader>
          <DialogBody className="space-y-4 text-[12px] leading-relaxed">
            {lastResult && <p>{resultMessage(lastResult.result, locale, lastResult.action)}</p>}
            {!!lastResult?.result.errors.length && (
              <ul className="space-y-2" aria-label={locale === "zh" ? "同步错误" : "Sync errors"}>
                {lastResult.result.errors.map((error, index) => (
                  <li
                    key={index}
                    className="break-words rounded-md border border-[var(--border-default)] bg-[var(--bg-surface)] p-3"
                  >
                    {error}
                  </li>
                ))}
              </ul>
            )}
          </DialogBody>
          <DialogFooter>
            <Button type="button" onClick={() => setDetailsOpen(false)}>
              {locale === "zh" ? "关闭" : "Close"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
