import { useState } from "react";
import { Database, RotateCcw } from "lucide-react";
import { compatApi, type SessionSyncResult } from "../lib/api/compat";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import { useAppDialog } from "./AppDialogProvider";

type Action = "sync" | "rebuild";

function resultMessage(result: SessionSyncResult, locale: string, action: Action) {
  const prefix =
    action === "rebuild"
      ? locale === "zh"
        ? "Codex 用量已重建"
        : "Codex usage rebuilt"
      : locale === "zh"
        ? "会话用量已同步"
        : "Session usage synced";
  return `${prefix}: ${result.imported} ${locale === "zh" ? "条新增" : "imported"}, ${result.suspectedDuplicates} ${locale === "zh" ? "条重复" : "duplicates"}`;
}

export default function SessionUsageActions() {
  const locale = getLocale();
  const appDialog = useAppDialog();
  const [busy, setBusy] = useState<Action | null>(null);

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
      showToast("success", resultMessage(result, locale, action));
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setBusy(null);
    }
  }

  return (
    <>
      <button className="btn btn-secondary btn-sm" onClick={() => void run("sync")} disabled={busy !== null}>
        <Database size={14} className={busy === "sync" ? "spin" : undefined} />
        {locale === "zh" ? "同步用量" : "Sync usage"}
      </button>
      <button className="btn btn-secondary btn-sm" onClick={() => void run("rebuild")} disabled={busy !== null}>
        <RotateCcw size={14} className={busy === "rebuild" ? "spin" : undefined} />
        {locale === "zh" ? "重建 Codex" : "Rebuild Codex"}
      </button>
    </>
  );
}
