import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import { Archive, CheckCircle, History, RefreshCw } from "lucide-react";
import ConfirmDialog from "./ConfirmDialog";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";

interface MigrationPreview {
  revision: string;
  sourceProviderIds: string[];
  targetProviderId: string;
  jsonlFiles: number;
  compressedFiles: number;
  stateRows: number;
}

interface CodexHistoryMigrationResult {
  sourceProviderIds: string[];
  targetProviderId: string;
  migratedJsonlFiles: number;
  migratedStateRows: number;
  backupPath: string | null;
  skippedReason: string | null;
}

export default function SettingsCodexHistorySection() {
  const queryClient = useQueryClient();
  const [phase, setPhase] = useState<"idle" | "checking" | "migrating">("idle");
  const [preview, setPreview] = useState<MigrationPreview | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [result, setResult] = useState<CodexHistoryMigrationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const check = async () => {
    if (busy.current) return;
    busy.current = true;
    setPhase("checking");
    setPreview(null);
    setResult(null);
    setError(null);
    try {
      const next = await invoke<MigrationPreview>("preview_codex_history_migration", {});
      if (mounted.current) setPreview(next);
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      busy.current = false;
      if (mounted.current) setPhase("idle");
    }
  };

  const migrate = async () => {
    if (busy.current || !preview) return;
    busy.current = true;
    setPhase("migrating");
    setResult(null);
    setError(null);
    try {
      const next = await invoke<CodexHistoryMigrationResult>("migrate_codex_history", {
        sourceProviderIds: preview.sourceProviderIds,
        targetProviderId: preview.targetProviderId,
        expectedRevision: preview.revision,
      });
      if (mounted.current) {
        setResult(next);
        setPreview(null);
      }
    } catch (reason) {
      if (mounted.current) {
        setError(String(reason));
        setPreview(null);
      }
    } finally {
      void queryClient.invalidateQueries({ queryKey: ["sessions"] });
      busy.current = false;
      if (mounted.current) {
        setPhase("idle");
        setConfirming(false);
      }
    }
  };

  const hasChanges = !!preview && (preview.jsonlFiles > 0 || preview.stateRows > 0);
  const running = phase !== "idle";
  return (
    <section className="section-card space-y-4" aria-labelledby="codex-history-migration-title">
      <div className="section-card-title" id="codex-history-migration-title">
        <History size={17} style={{ color: "var(--text-secondary)" }} />
        Codex 历史会话
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        先检查旧配置关联的历史，再将它们归到同一个分桶，减少配置切换后会话分散的问题。 支持普通和压缩日志。迁移前请关闭
        Codex 客户端；最近仍在更新的会话会停止迁移。
      </p>
      <div className="flex flex-wrap gap-2">
        <Button variant="secondary" onClick={() => void check()} disabled={running || confirming}>
          <RefreshCw size={14} className={running ? "spin" : ""} aria-hidden="true" />
          {phase === "checking" ? "检查中…" : "检查历史"}
        </Button>
        {hasChanges && (
          <Button onClick={() => setConfirming(true)} disabled={running || confirming}>
            迁移已检查的历史
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="break-words text-xs leading-relaxed text-[var(--danger)]">
          {error}
        </p>
      )}
      {preview && (
        <Card role="status">
          <CardHeader>
            <CardTitle>{hasChanges ? "检查完成，等待迁移" : "没有需要迁移的历史"}</CardTitle>
            <CardDescription>检查不会修改文件。仅迁移所列来源，其他会话和配置保持原样。</CardDescription>
          </CardHeader>
          {hasChanges && (
            <CardContent className="space-y-3 text-xs">
              <dl className="grid grid-cols-2 gap-2 text-muted-foreground">
                <dt>会话日志</dt>
                <dd className="text-right text-foreground">
                  {preview.jsonlFiles} 个（压缩 {preview.compressedFiles} 个）
                </dd>
                <dt>状态记录</dt>
                <dd className="text-right text-foreground">{preview.stateRows} 条</dd>
                <dt>目标分桶</dt>
                <dd className="break-all text-right text-foreground">{preview.targetProviderId}</dd>
              </dl>
              <p className="break-words text-muted-foreground">来源：{preview.sourceProviderIds.join("、")}</p>
            </CardContent>
          )}
        </Card>
      )}
      {result && (
        <div role="status" className="space-y-2 text-xs text-muted-foreground">
          <p className="flex items-start gap-2 text-[var(--success)]">
            <CheckCircle size={14} className="shrink-0" aria-hidden="true" />
            已迁移 {result.migratedJsonlFiles} 个会话文件、{result.migratedStateRows} 条状态记录
          </p>
          {result.backupPath && (
            <p className="flex items-start gap-2 break-all">
              <Archive size={14} className="shrink-0" aria-hidden="true" />
              原始备份：{result.backupPath}
            </p>
          )}
        </div>
      )}
      <ConfirmDialog
        isOpen={confirming}
        title="迁移已检查的历史？"
        variant="info"
        message={`将迁移 ${preview?.jsonlFiles ?? 0} 个会话文件和 ${preview?.stateRows ?? 0} 条状态记录到 ${preview?.targetProviderId ?? ""}。\n执行前会保留原始日志和完整状态数据库备份。检查后发生变化时停止，请重新检查。`}
        confirmText={phase === "migrating" ? "迁移中…" : "创建备份并迁移"}
        busy={phase === "migrating"}
        onConfirm={() => void migrate()}
        onCancel={() => setConfirming(false)}
      />
    </section>
  );
}
