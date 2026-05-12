import ConfirmDialog from "../../components/ConfirmDialog";
import { showToast } from "../../components/Toast";
import type { Locale } from "../../lib/i18n";
import type { Plugin, Skill, SkillBackup } from "./helpers";

type PendingDelete = { type: "skill"; item: Skill } | { type: "plugin"; item: Plugin };

interface SkillsConfirmDialogsProps {
  pendingDelete: PendingDelete | null;
  setPendingDelete: React.Dispatch<React.SetStateAction<PendingDelete | null>>;
  pendingBackupDelete: SkillBackup | null;
  setPendingBackupDelete: React.Dispatch<React.SetStateAction<SkillBackup | null>>;
  setBackupBusyId: React.Dispatch<React.SetStateAction<string | null>>;
  doDeletePlugin: (plugin: Plugin) => void;
  doDeleteSkill: (skill: Skill) => void;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  deleteSkillBackupMutation: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  applySkillsPageData: (data: any) => void;
  locale: Locale;
}

export default function SkillsConfirmDialogs({
  pendingDelete,
  setPendingDelete,
  pendingBackupDelete,
  setPendingBackupDelete,
  setBackupBusyId,
  doDeletePlugin,
  doDeleteSkill,
  deleteSkillBackupMutation,
  applySkillsPageData,
  locale,
}: SkillsConfirmDialogsProps) {
  return (
    <>
      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={
          pendingDelete?.type === "plugin"
            ? locale === "zh"
              ? "删除插件"
              : "Delete Plugin"
            : locale === "zh"
              ? "删除技能"
              : "Delete Skill"
        }
        message={
          pendingDelete?.type === "plugin"
            ? locale === "zh"
              ? `确定删除插件「${pendingDelete?.item.name}」？此操作不可恢复。`
              : `Delete plugin "${pendingDelete?.item.name}"? This cannot be undone.`
            : locale === "zh"
              ? `确定删除技能「${pendingDelete?.item.name}」？`
              : `Delete skill "${pendingDelete?.item.name}"?`
        }
        confirmText={locale === "zh" ? "删除" : "Delete"}
        variant="destructive"
        onConfirm={() => {
          if (!pendingDelete) return;
          if (pendingDelete.type === "plugin") void doDeletePlugin(pendingDelete.item as Plugin);
          else void doDeleteSkill(pendingDelete.item as Skill);
          setPendingDelete(null);
        }}
        onCancel={() => setPendingDelete(null)}
      />

      <ConfirmDialog
        isOpen={!!pendingBackupDelete}
        title={locale === "zh" ? "删除 Skill 备份" : "Delete Skill Backup"}
        message={
          pendingBackupDelete
            ? locale === "zh"
              ? `确定删除备份「${pendingBackupDelete.skill_name}」？删除后将无法再从该备份恢复。`
              : `Delete backup "${pendingBackupDelete.skill_name}"? This backup cannot be restored after deletion.`
            : ""
        }
        confirmText={locale === "zh" ? "删除备份" : "Delete Backup"}
        variant="destructive"
        onConfirm={() => {
          const backup = pendingBackupDelete;
          if (!backup) return;
          setPendingBackupDelete(null);
          setBackupBusyId(backup.id);
          void deleteSkillBackupMutation
            .mutateAsync({ id: backup.id })
            .then((data: unknown) => applySkillsPageData(data))
            .then(() => showToast("success", locale === "zh" ? "备份已删除" : "Backup deleted"))
            .catch((error: unknown) => showToast("error", String(error)))
            .finally(() => setBackupBusyId((current) => (current === backup.id ? null : current)));
        }}
        onCancel={() => setPendingBackupDelete(null)}
      />
    </>
  );
}
