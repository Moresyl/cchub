import { memo } from "react";
import { FolderOpen } from "lucide-react";
import type { Locale } from "../lib/i18n";

export interface SettingsManagedBackupRowBackup {
  path: string;
  name: string;
  created_at: string;
  size_bytes: number;
  kind: string;
  can_restore: boolean;
}

interface SettingsManagedBackupRowProps {
  backup: SettingsManagedBackupRowBackup;
  locale: Locale;
  isRestoring: boolean;
  isDeleting: boolean;
  onRename: (backup: SettingsManagedBackupRowBackup) => void | Promise<void>;
  onOpen: (target: string, label: string) => void | Promise<void>;
  onRestore: (backup: SettingsManagedBackupRowBackup) => void | Promise<void>;
  onDelete: (backup: SettingsManagedBackupRowBackup) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function formatBytes(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

function SettingsManagedBackupRowComponent({
  backup,
  locale,
  isRestoring,
  isDeleting,
  onRename,
  onOpen,
  onRestore,
  onDelete,
}: SettingsManagedBackupRowProps) {
  return (
    <div
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-input)",
        display: "flex",
        justifyContent: "space-between",
        gap: 12,
        alignItems: "center",
        flexWrap: "wrap",
      }}
    >
      <div style={{ minWidth: 0, flex: 1 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginBottom: 6 }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{backup.name}</span>
          <span className={`badge ${backup.kind === "scheduled" ? "badge-warning" : "badge-accent"}`} style={{ fontSize: 10 }}>
            {backup.kind === "scheduled"
              ? uiText(locale, "自动备份", "Scheduled", "自動バックアップ")
              : uiText(locale, "手动备份", "Manual", "手動バックアップ")}
          </span>
          <span className="badge badge-muted" style={{ fontSize: 10 }}>{formatBytes(backup.size_bytes)}</span>
        </div>
        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
          {backup.created_at.replace("T", " ").slice(0, 19)}
        </div>
        <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
          {backup.path}
        </div>
      </div>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        <button className="btn btn-secondary btn-sm" onClick={() => onRename(backup)}>
          {uiText(locale, "重命名", "Rename", "名前変更")}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => onOpen(backup.path, backup.name)}
          style={{ gap: 6 }}
        >
          <FolderOpen size={14} />
          {uiText(locale, "打开", "Open", "開く")}
        </button>
        <button
          className="btn btn-secondary btn-sm"
          disabled={!backup.can_restore || isRestoring}
          onClick={() => void onRestore(backup)}
        >
          {isRestoring
            ? uiText(locale, "恢复中...", "Restoring...", "復元中...")
            : uiText(locale, "恢复", "Restore", "復元")}
        </button>
        <button
          className="btn btn-danger-ghost btn-sm"
          disabled={isDeleting}
          onClick={() => void onDelete(backup)}
        >
          {isDeleting
            ? uiText(locale, "删除中...", "Deleting...", "削除中...")
            : uiText(locale, "删除", "Delete", "削除")}
        </button>
      </div>
    </div>
  );
}

export default memo(SettingsManagedBackupRowComponent);
