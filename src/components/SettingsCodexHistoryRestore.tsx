import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import { ArchiveRestore, CheckCircle, Search } from "lucide-react";
import ConfirmDialog from "./ConfirmDialog";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Checkbox } from "./ui/checkbox";
import { Input } from "./ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

interface Backup {
  key: string;
  createdAtSeconds: number;
  targetProviderId: string | null;
  logFiles: number;
  stateRows: number;
  problem: string | null;
}

interface RestoreItem {
  key: string;
  sessionId: string;
  originalProviderId: string;
  status: "ready" | "conflict" | "restored";
  reason: string | null;
  logFiles: number;
  stateRows: number;
}

interface Preview {
  backupKey: string;
  revision: string;
  items: RestoreItem[];
}

interface Result {
  restoredJsonlFiles: number;
  restoredStateRows: number;
  backupPath: string;
}

const PAGE_SIZE = 40;
const statusText = { ready: "可恢复", conflict: "需要检查", restored: "已恢复" };
function backupLabel(backup: Backup) {
  const date = backup.createdAtSeconds ? new Date(backup.createdAtSeconds * 1000).toLocaleString() : "时间未知";
  return `${date} · ${backup.logFiles} 个日志 / ${backup.stateRows} 条状态 · ${backup.targetProviderId ?? "无效备份"}`;
}

export default function SettingsCodexHistoryRestore({
  disabled,
  onBusyChange,
  onRestored,
}: {
  disabled: boolean;
  onBusyChange: (busy: boolean) => void;
  onRestored: () => void;
}) {
  const queryClient = useQueryClient();
  const [phase, setPhase] = useState<"idle" | "listing" | "checking" | "restoring">("idle");
  const [backups, setBackups] = useState<Backup[] | null>(null);
  const [choice, setChoice] = useState("");
  const [preview, setPreview] = useState<Preview | null>(null);
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const [result, setResult] = useState<Result | null>(null);
  const [error, setError] = useState<string | null>(null);
  const busy = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    onBusyChange(phase !== "idle" || confirming);
  }, [phase, confirming, onBusyChange]);

  const run = async (operation: "listing" | "checking" | "restoring") => {
    if (busy.current || disabled || (confirming && operation !== "restoring")) return;
    busy.current = true;
    setPhase(operation);
    setError(null);
    setResult(null);
    const checked = preview;
    if (operation === "listing") {
      setBackups(null);
      setChoice("");
    }
    if (operation !== "restoring") {
      setPreview(null);
      setSelection(new Set());
    }
    setSearch("");
    setPage(0);
    try {
      if (operation === "listing") {
        const next = await invoke<Backup[]>("list_codex_history_migration_backups", {});
        if (mounted.current) {
          setBackups(next);
          setChoice(next.find((backup) => !backup.problem)?.key ?? "");
        }
      } else if (operation === "checking") {
        const next = await invoke<Preview>("preview_codex_history_restore", { backupKey: choice });
        if (mounted.current) {
          setPreview(next);
          setSelection(new Set(next.items.filter((item) => item.status === "ready").map((item) => item.key)));
        }
      } else if (checked) {
        const next = await invoke<Result>("restore_codex_history_migration", {
          backupKey: checked.backupKey,
          expectedRevision: checked.revision,
          selectedKeys: [...selection],
        });
        if (mounted.current) {
          setResult(next);
          onRestored();
        }
      }
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      if (operation === "restoring") {
        void queryClient.invalidateQueries({ queryKey: ["sessions"] });
        if (mounted.current) {
          setPreview(null);
          setSelection(new Set());
        }
      }
      busy.current = false;
      if (mounted.current) {
        setPhase("idle");
        setConfirming(false);
      }
    }
  };

  const locked = disabled || phase !== "idle" || confirming;
  const ready = preview?.items.filter((item) => item.status === "ready") ?? [];
  const selected = ready.filter((item) => selection.has(item.key));
  const logFiles = selected.reduce((count, item) => count + item.logFiles, 0);
  const stateRows = selected.reduce((count, item) => count + item.stateRows, 0);
  const filtered =
    preview?.items.filter((item) =>
      `${item.sessionId} ${item.originalProviderId} ${item.reason ?? ""}`.toLowerCase().includes(search.toLowerCase()),
    ) ?? [];
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const currentPage = Math.min(page, pages - 1);
  return (
    <div className="space-y-4 border-t border-[var(--border-subtle)] pt-4">
      <div className="space-y-2">
        <h3 className="text-sm font-semibold">恢复迁移前的配置归属</h3>
        <p className="text-xs leading-relaxed text-muted-foreground">
          从当前配置目录的迁移备份中选择会话。仅还原分桶标识，保留后来新增的消息、最新标题和其他状态记录。
          同一会话的日志与状态一起恢复；存在冲突时不能选择。请先关闭 Codex 客户端。
        </p>
        <Button variant="secondary" disabled={locked} onClick={() => void run("listing")}>
          <ArchiveRestore size={14} aria-hidden="true" />
          {phase === "listing" ? "检查备份中…" : "恢复迁移备份"}
        </Button>
      </div>
      {backups && backups.length === 0 && (
        <p role="status" className="text-xs text-muted-foreground">
          当前配置目录没有迁移备份。
        </p>
      )}
      {backups && backups.length > 0 && (
        <div className="space-y-2">
          <label id="codex-restore-backup-label" className="text-xs text-muted-foreground">
            选择迁移备份
          </label>
          <div className="flex flex-wrap gap-2">
            <Select
              value={choice}
              disabled={locked}
              onValueChange={(value) => {
                setChoice(value);
                setPreview(null);
                setSelection(new Set());
                setError(null);
                setResult(null);
              }}
            >
              <SelectTrigger aria-labelledby="codex-restore-backup-label" className="min-w-0 flex-1 basis-64">
                <SelectValue placeholder="选择有效的迁移备份" />
              </SelectTrigger>
              <SelectContent>
                {backups.map((backup) => (
                  <SelectItem key={backup.key} value={backup.key} disabled={!!backup.problem}>
                    {backupLabel(backup)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button variant="secondary" disabled={locked || !choice} onClick={() => void run("checking")}>
              {phase === "checking" ? "检查中…" : "预览恢复范围"}
            </Button>
          </div>
          {backups
            .filter((backup) => backup.problem)
            .map((backup) => (
              <p key={backup.key} className="break-words text-xs text-[var(--warning)]">
                有一份备份无法使用：{backup.problem}
              </p>
            ))}
        </div>
      )}
      {error && (
        <p role="alert" className="break-words text-xs leading-relaxed text-[var(--danger)]">
          {error}
        </p>
      )}
      {preview && (
        <Card>
          <CardHeader>
            <CardTitle>恢复预览</CardTitle>
            <CardDescription>
              可恢复 {ready.length} 个会话 · 需要检查{" "}
              {preview.items.filter((item) => item.status === "conflict").length} 个 · 已恢复{" "}
              {preview.items.filter((item) => item.status === "restored").length} 个
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3 text-xs">
            {preview.items.length === 0 ? (
              <p role="status">这份备份没有会话记录。</p>
            ) : (
              <>
                <div className="relative">
                  <Search
                    size={14}
                    className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground"
                    aria-hidden="true"
                  />
                  <Input
                    aria-label="搜索恢复会话"
                    className="pl-7"
                    value={search}
                    disabled={locked}
                    placeholder="搜索会话 ID 或原始分桶…"
                    onChange={(event) => {
                      setSearch(event.target.value);
                      setPage(0);
                    }}
                  />
                </div>
                <label className="flex items-center gap-2 text-muted-foreground">
                  <Checkbox
                    aria-label="选择全部可恢复会话"
                    disabled={locked || ready.length === 0}
                    checked={selected.length === 0 ? false : selected.length === ready.length ? true : "indeterminate"}
                    onCheckedChange={(checked) => setSelection(new Set(checked ? ready.map((item) => item.key) : []))}
                  />
                  选择全部可恢复会话（{ready.length} 个）
                </label>
                <ul className="max-h-80 space-y-2 overflow-y-auto overscroll-contain" aria-label="恢复会话列表">
                  {filtered.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE).map((item) => (
                    <li
                      key={item.key}
                      className="flex items-start gap-3 rounded-lg border border-[var(--border-subtle)] p-3"
                    >
                      <Checkbox
                        aria-label={`恢复会话 ${item.sessionId} 到 ${item.originalProviderId}`}
                        disabled={locked || item.status !== "ready"}
                        checked={selection.has(item.key)}
                        onCheckedChange={(checked) =>
                          setSelection((previous) => {
                            const next = new Set(previous);
                            if (checked) next.add(item.key);
                            else next.delete(item.key);
                            return next;
                          })
                        }
                      />
                      <div className="min-w-0 flex-1 space-y-1">
                        <p className="break-all text-foreground">{item.sessionId}</p>
                        <p className="break-words text-muted-foreground">
                          原始分桶：{item.originalProviderId} · {statusText[item.status]}
                        </p>
                        {item.status === "ready" && (
                          <p className="text-muted-foreground">
                            {item.logFiles} 个日志 · {item.stateRows} 条状态
                          </p>
                        )}
                        {item.reason && <p className="break-words text-[var(--warning)]">{item.reason}</p>}
                      </div>
                    </li>
                  ))}
                </ul>
                {filtered.length === 0 && (
                  <p role="status" className="text-muted-foreground">
                    没有匹配的会话。
                  </p>
                )}
                {pages > 1 && (
                  <div className="flex items-center justify-between gap-2">
                    <Button
                      variant="ghost"
                      disabled={locked || currentPage === 0}
                      onClick={() => setPage(currentPage - 1)}
                    >
                      上一页
                    </Button>
                    <span className="text-muted-foreground">
                      {currentPage + 1} / {pages}
                    </span>
                    <Button
                      variant="ghost"
                      disabled={locked || currentPage === pages - 1}
                      onClick={() => setPage(currentPage + 1)}
                    >
                      下一页
                    </Button>
                  </div>
                )}
                <div className="flex flex-wrap items-center justify-between gap-3 border-t border-[var(--border-subtle)] pt-3">
                  <p role="status" className="text-muted-foreground">
                    已选 {selected.length} 个会话 · {logFiles} 个日志 · {stateRows} 条状态
                  </p>
                  <Button disabled={locked || selected.length === 0} onClick={() => setConfirming(true)}>
                    恢复所选会话
                  </Button>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      )}
      {result && (
        <div role="status" className="space-y-2 text-xs">
          <p className="flex items-start gap-2 text-[var(--success)]">
            <CheckCircle size={14} className="shrink-0" aria-hidden="true" />
            已恢复 {result.restoredJsonlFiles} 个日志、{result.restoredStateRows} 条状态记录
          </p>
          <p className="break-all text-muted-foreground">恢复前的安全备份：{result.backupPath}</p>
        </div>
      )}
      <ConfirmDialog
        isOpen={confirming}
        title="恢复所选会话的配置归属？"
        variant="info"
        message={`将恢复 ${selected.length} 个会话的配置归属。只更新分桶标识，保留现有消息和其他字段。\n执行前会创建安全备份；预览后发生变化时停止并要求重新检查。`}
        confirmText={phase === "restoring" ? "恢复中…" : "创建备份并恢复"}
        busy={phase === "restoring"}
        onConfirm={() => void run("restoring")}
        onCancel={() => setConfirming(false)}
      />
    </div>
  );
}
