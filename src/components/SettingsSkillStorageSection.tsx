import { useCallback, useEffect, useState } from "react";
import { FolderSync, Loader2, ShieldCheck } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "./Toast";
import { getLocale } from "../lib/i18n";

type SkillStorageLocation = "cchub" | "unified" | "tool";

interface MigrationResult {
  migrated?: number;
  skipped?: number;
  errors?: string[];
  location?: SkillStorageLocation;
}

const text = (locale: string, zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);

export default function SettingsSkillStorageSection() {
  const locale = getLocale();
  const [location, setLocation] = useState<SkillStorageLocation>("tool");
  const [installedCount, setInstalledCount] = useState(0);
  const [pendingTarget, setPendingTarget] = useState<Exclude<SkillStorageLocation, "tool"> | null>(null);
  const [loading, setLoading] = useState(true);
  const [migrating, setMigrating] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [current, skills] = await Promise.all([
        invoke<string>("get_skill_storage_location"),
        invoke<unknown[]>("scan_skills"),
      ]);
      if (current === "cchub" || current === "unified" || current === "tool") {
        setLocation(current);
      }
      setInstalledCount(Array.isArray(skills) ? skills.length : 0);
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const migrate = useCallback(
    async (target: Exclude<SkillStorageLocation, "tool">) => {
      setMigrating(true);
      try {
        const result = await invoke<MigrationResult>("migrate_skill_storage", {
          target: { kind: target },
        });
        if (result.location === "cchub" || result.location === "unified" || result.location === "tool") {
          setLocation(result.location);
        }
        setPendingTarget(null);
        const migrated = result?.migrated ?? 0;
        const skipped = result?.skipped ?? 0;
        if ((result?.errors?.length ?? 0) > 0) {
          showToast(
            "error",
            text(
              locale,
              `已迁移 ${migrated} 项，跳过 ${skipped} 项，部分文件失败`,
              `${migrated} migrated, ${skipped} skipped; some files failed`,
              `${migrated} 件を移行、${skipped} 件をスキップ。一部失敗しました`,
            ),
          );
        } else {
          showToast(
            "success",
            text(
              locale,
              `技能存储已迁移（${migrated} 项）`,
              `Skill storage migrated (${migrated})`,
              `スキル保存先を移行しました（${migrated} 件）`,
            ),
          );
        }
        await load();
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setMigrating(false);
      }
    },
    [load, locale],
  );

  const choose = (target: Exclude<SkillStorageLocation, "tool">) => {
    if (location === target) return;
    if (installedCount > 0) {
      setPendingTarget(target);
      return;
    }
    void migrate(target);
  };

  return (
    <section className="section-card" aria-labelledby="skill-storage-title">
      <div className="section-card-title" id="skill-storage-title">
        <FolderSync size={17} style={{ color: "var(--text-secondary)" }} />
        {text(locale, "技能存储位置", "Skill storage", "スキル保存先")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", margin: "0 0 14px" }}>
        {text(
          locale,
          "统一管理目录后，所有工具的技能安装、扫描和备份都会使用同一份文件。",
          "A shared directory keeps skill installs, scans, and backups consistent across tools.",
          "共有ディレクトリを使うと、各ツールのインストール、スキャン、バックアップが同じファイルを参照します。",
        )}
      </p>
      {loading ? (
        <div style={{ color: "var(--text-muted)", display: "flex", alignItems: "center", gap: 8 }}>
          <Loader2 size={15} className="spin" />
          {text(locale, "正在读取技能存储设置…", "Loading skill storage…", "保存先を読み込み中…")}
        </div>
      ) : (
        <>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button
              type="button"
              className={`btn btn-sm ${location === "cchub" ? "btn-primary" : "btn-secondary"}`}
              disabled={migrating}
              onClick={() => choose("cchub")}
            >
              {text(locale, "CCHub 集中目录", "CCHub directory", "CCHub 集中ディレクトリ")}
            </button>
            <button
              type="button"
              className={`btn btn-sm ${location === "unified" ? "btn-primary" : "btn-secondary"}`}
              disabled={migrating}
              onClick={() => choose("unified")}
            >
              {text(locale, "统一 Agent 目录", "Unified agent directory", "統合 Agent ディレクトリ")}
            </button>
          </div>
          <div
            style={{
              display: "flex",
              gap: 7,
              alignItems: "center",
              marginTop: 10,
              fontSize: 11,
              color: "var(--text-muted)",
            }}
          >
            <ShieldCheck size={13} />
            {location === "tool"
              ? text(
                  locale,
                  `当前按各工具目录保存（${installedCount} 项）`,
                  `Currently using per-tool directories (${installedCount})`,
                  `現在はツール別ディレクトリを使用（${installedCount} 件）`,
                )
              : text(
                  locale,
                  `当前已统一保存（${installedCount} 项）`,
                  `Shared storage is active (${installedCount})`,
                  `共有保存先が有効（${installedCount} 件）`,
                )}
          </div>
        </>
      )}
      {pendingTarget && (
        <div className="section-card" style={{ marginTop: 14, padding: 12, borderColor: "var(--warning)" }}>
          <p style={{ margin: 0, fontSize: 12, lineHeight: 1.6 }}>
            {text(
              locale,
              `将迁移现有 ${installedCount} 项技能并切换存储目录，原文件会在成功后移除。继续吗？`,
              `This will migrate ${installedCount} skills and remove the old files after success. Continue?`,
              `${installedCount} 件のスキルを移行し、成功後に旧ファイルを削除します。続行しますか？`,
            )}
          </p>
          <div style={{ display: "flex", gap: 8, marginTop: 10, justifyContent: "flex-end" }}>
            <button type="button" className="btn btn-sm btn-secondary" onClick={() => setPendingTarget(null)}>
              {text(locale, "取消", "Cancel", "キャンセル")}
            </button>
            <button type="button" className="btn btn-sm btn-primary" onClick={() => void migrate(pendingTarget)}>
              {migrating ? <Loader2 size={13} className="spin" /> : null}
              {text(locale, "确认迁移", "Migrate", "移行")}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
