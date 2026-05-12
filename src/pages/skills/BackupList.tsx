import { RotateCcw, Trash2 } from "lucide-react";

import { showToast } from "../../components/Toast";
import type { Locale } from "../../lib/i18n";
import type { SkillBackup } from "./helpers";

interface SkillsBackupListProps {
  skillBackups: SkillBackup[];
  backupBusyId: string | null;
  setBackupBusyId: React.Dispatch<React.SetStateAction<string | null>>;
  setPendingBackupDelete: React.Dispatch<React.SetStateAction<SkillBackup | null>>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  restoreSkillBackupMutation: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  applySkillsPageData: (data: any) => void;
  locale: Locale;
}

export default function SkillsBackupList({
  skillBackups,
  backupBusyId,
  setBackupBusyId,
  setPendingBackupDelete,
  restoreSkillBackupMutation,
  applySkillsPageData,
  locale,
}: SkillsBackupListProps) {
  if (skillBackups.length === 0) return null;
  return (
    <div className="section-card" style={{ marginBottom: 16 }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 12,
          alignItems: "center",
          marginBottom: 12,
          flexWrap: "wrap",
        }}
      >
        <div>
          <div className="section-card-title" style={{ marginBottom: 4 }}>
            <RotateCcw size={16} style={{ color: "var(--text-secondary)" }} />
            {locale === "zh" ? "Skill 备份" : "Skill Backups"}
          </div>
          <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
            {locale === "zh"
              ? `自动保留最近 ${skillBackups.length} 个卸载备份，可恢复或删除`
              : `Recent uninstall backups are kept automatically. You can restore or delete them here.`}
          </p>
        </div>
        <span className="badge badge-accent">{skillBackups.length}</span>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {skillBackups.slice(0, 8).map((backup) => (
          <div
            key={backup.id}
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
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap", marginBottom: 4 }}>
                <span style={{ fontSize: 13, fontWeight: 600 }}>{backup.skill_name}</span>
                <span className="badge badge-muted" style={{ fontSize: 10 }}>
                  {backup.size_bytes < 1024 ? `${backup.size_bytes} B` : `${(backup.size_bytes / 1024).toFixed(1)} KB`}
                </span>
              </div>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  fontFamily: "'JetBrains Mono', monospace",
                  wordBreak: "break-all",
                }}
              >
                {backup.original_path}
              </div>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
                {backup.created_at.replace("T", " ").slice(0, 19)}
              </div>
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <button
                className="btn btn-secondary btn-sm"
                disabled={backupBusyId === backup.id}
                onClick={async () => {
                  setBackupBusyId(backup.id);
                  try {
                    const { restoredTo, data } = await restoreSkillBackupMutation.mutateAsync({ id: backup.id });
                    applySkillsPageData(data);
                    showToast("success", locale === "zh" ? `已恢复到 ${restoredTo}` : `Restored to ${restoredTo}`);
                  } catch (e) {
                    showToast("error", String(e));
                  } finally {
                    setBackupBusyId((current) => (current === backup.id ? null : current));
                  }
                }}
                style={{ gap: 6 }}
              >
                <RotateCcw size={13} />
                {locale === "zh" ? "恢复" : "Restore"}
              </button>
              <button
                className="btn btn-danger-ghost btn-sm"
                disabled={backupBusyId === backup.id}
                onClick={() => setPendingBackupDelete(backup)}
                style={{ gap: 6 }}
              >
                <Trash2 size={13} />
                {locale === "zh" ? "删除备份" : "Delete Backup"}
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
