import { memo, type ChangeEventHandler } from "react";
import { Archive, RefreshCw } from "lucide-react";
import type { Locale } from "../lib/i18n";
import SettingsManagedBackupRow, { type SettingsManagedBackupRowBackup } from "./SettingsManagedBackupRow";
import EmptyState from "./states/EmptyState";
import LoadingState from "./states/LoadingState";

interface SettingsBackupManagementSectionProps {
  locale: Locale;
  autoBackupEnabled: boolean;
  retentionCount: number;
  savingBackupPreferences: boolean;
  creatingManagedBackup: boolean;
  loadingManagedBackups: boolean;
  managedBackups: SettingsManagedBackupRowBackup[];
  restoringBackupPath: string | null;
  deletingBackupPath: string | null;
  onToggleAutoBackup: () => void | Promise<void>;
  onRetentionChange: ChangeEventHandler<HTMLInputElement>;
  onSavePreferences: () => void | Promise<void>;
  onCreateManagedBackup: () => void | Promise<void>;
  onRefreshManagedBackups: () => void | Promise<void>;
  onRenameManagedBackup: (backup: SettingsManagedBackupRowBackup) => void | Promise<void>;
  onOpenBackup: (target: string, label: string) => void | Promise<void>;
  onRestoreManagedBackup: (backup: SettingsManagedBackupRowBackup) => void | Promise<void>;
  onDeleteManagedBackup: (backup: SettingsManagedBackupRowBackup) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsBackupManagementSectionComponent({
  locale,
  autoBackupEnabled,
  retentionCount,
  savingBackupPreferences,
  creatingManagedBackup,
  loadingManagedBackups,
  managedBackups,
  restoringBackupPath,
  deletingBackupPath,
  onToggleAutoBackup,
  onRetentionChange,
  onSavePreferences,
  onCreateManagedBackup,
  onRefreshManagedBackups,
  onRenameManagedBackup,
  onOpenBackup,
  onRestoreManagedBackup,
  onDeleteManagedBackup,
}: SettingsBackupManagementSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Archive size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "备份管理", "Backup Management", "バックアップ管理")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
        {uiText(
          locale,
          "集中管理 SQL 备份，支持手动创建、每小时自动备份、保留策略、重命名、删除与恢复。",
          "Manage SQL backups in one place, including manual creation, hourly automatic backups, retention, rename, delete, and restore.",
          "SQL バックアップを一元管理します。手動作成、毎時自動バックアップ、保持数、名前変更、削除、復元に対応します。",
        )}
      </p>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
          gap: 12,
          marginBottom: 16,
        }}
      >
        <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText(locale, "每小时自动备份", "Hourly Auto Backup", "毎時自動バックアップ")}
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center" }}>
            <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
              {autoBackupEnabled
                ? uiText(
                    locale,
                    "已启用，启动时会补齐缺失的小时备份",
                    "Enabled. Missing hourly backups are created on startup.",
                    "有効です。不足している毎時バックアップは起動時に作成されます。",
                  )
                : uiText(locale, "未启用", "Disabled", "無効")}
            </div>
            <button
              className={`toggle ${autoBackupEnabled ? "on" : "off"}`}
              onClick={onToggleAutoBackup}
              disabled={savingBackupPreferences}
            >
              <div className="toggle-knob" />
            </button>
          </div>
        </div>

        <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText(locale, "保留最近备份数", "Retention Count", "保持する最近のバックアップ数")}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <input
              className="input"
              type="number"
              min={1}
              max={99}
              value={retentionCount}
              onChange={onRetentionChange}
              style={{ maxWidth: 110 }}
            />
            <button className="btn btn-secondary btn-sm" onClick={onSavePreferences} disabled={savingBackupPreferences}>
              {savingBackupPreferences
                ? uiText(locale, "保存中...", "Saving...", "保存中...")
                : uiText(locale, "保存策略", "Save", "保存")}
            </button>
          </div>
        </div>

        <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>
            {uiText(locale, "受管备份", "Managed Backups", "管理対象バックアップ")}
          </div>
          <div style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>{managedBackups.length}</div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button
              className="btn btn-primary btn-sm"
              onClick={onCreateManagedBackup}
              disabled={creatingManagedBackup}
              style={{ gap: 6 }}
            >
              <Archive size={14} className={creatingManagedBackup ? "spin" : ""} />
              {creatingManagedBackup
                ? uiText(locale, "创建中...", "Creating...", "作成中...")
                : uiText(locale, "立即备份", "Create Backup", "今すぐバックアップ")}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              onClick={onRefreshManagedBackups}
              disabled={loadingManagedBackups}
              style={{ gap: 6 }}
            >
              <RefreshCw size={14} className={loadingManagedBackups ? "spin" : ""} />
              {uiText(locale, "刷新列表", "Refresh", "一覧を更新")}
            </button>
          </div>
        </div>
      </div>

      {loadingManagedBackups ? (
        <LoadingState
          label={uiText(locale, "正在读取备份列表...", "Loading backup list...", "バックアップ一覧を読み込み中...")}
        />
      ) : managedBackups.length === 0 ? (
        <EmptyState
          title={uiText(locale, "暂无托管备份", "No managed backups yet", "管理対象バックアップはまだありません")}
          description={uiText(
            locale,
            "点击“立即备份”创建第一份 SQL 备份。",
            "Create your first SQL backup from here.",
            "ここから最初の SQL バックアップを作成できます。",
          )}
          icon={<Archive size={26} style={{ color: "var(--text-muted)" }} />}
        />
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {managedBackups.map((backup) => (
            <SettingsManagedBackupRow
              key={backup.path}
              backup={backup}
              locale={locale}
              isRestoring={restoringBackupPath === backup.path}
              isDeleting={deletingBackupPath === backup.path}
              onRename={onRenameManagedBackup}
              onOpen={onOpenBackup}
              onRestore={onRestoreManagedBackup}
              onDelete={onDeleteManagedBackup}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default memo(SettingsBackupManagementSectionComponent);
