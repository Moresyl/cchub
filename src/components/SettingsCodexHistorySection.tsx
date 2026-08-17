import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Archive, CheckCircle, History, RefreshCw } from "lucide-react";

interface CodexHistoryMigrationResult {
  sourceProviderIds: string[];
  targetProviderId: string;
  migratedJsonlFiles: number;
  migratedStateRows: number;
  backupPath: string | null;
  skippedReason: string | null;
}

export default function SettingsCodexHistorySection() {
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CodexHistoryMigrationResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const migrate = useCallback(async () => {
    setRunning(true);
    setError(null);
    try {
      const next = await invoke<CodexHistoryMigrationResult>("migrate_codex_history", {});
      setResult(next);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setRunning(false);
    }
  }, []);

  const migrated = result && (result.migratedJsonlFiles > 0 || result.migratedStateRows > 0);
  return (
    <section className="section-card" aria-labelledby="codex-history-migration-title">
      <div className="section-card-title" id="codex-history-migration-title">
        <History size={17} style={{ color: "var(--text-secondary)" }} />
        Codex 历史会话
      </div>
      <p style={{ color: "var(--text-secondary)", fontSize: 13, lineHeight: 1.55 }}>
        将旧 provider 分桶安全迁移到 custom。执行前会创建备份，可避免切换 provider 后历史会话分裂。
      </p>
      <button
        className="btn btn-secondary btn-sm"
        onClick={() => void migrate()}
        disabled={running}
        style={{ alignSelf: "flex-start", display: "inline-flex", gap: 6 }}
      >
        <RefreshCw size={14} className={running ? "spin" : ""} />
        {running ? "迁移中..." : "检查并迁移历史"}
      </button>
      {error && <p style={{ color: "var(--danger)", fontSize: 12 }}>{error}</p>}
      {result && (
        <div style={{ display: "flex", flexDirection: "column", gap: 5, fontSize: 12, color: "var(--text-secondary)" }}>
          {migrated ? (
            <span style={{ color: "var(--success)", display: "inline-flex", gap: 6, alignItems: "center" }}>
              <CheckCircle size={14} /> 已迁移 {result.migratedJsonlFiles} 个会话文件、{result.migratedStateRows}{" "}
              条状态记录
            </span>
          ) : (
            <span>
              {result.skippedReason === "no_source_provider_ids" ? "未发现可迁移的 provider" : "历史已经是最新分桶"}
            </span>
          )}
          {result.backupPath && (
            <span style={{ display: "inline-flex", gap: 6, alignItems: "center", wordBreak: "break-all" }}>
              <Archive size={14} /> 备份：{result.backupPath}
            </span>
          )}
        </div>
      )}
    </section>
  );
}
