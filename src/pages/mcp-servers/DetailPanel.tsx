import { lazy, useEffect, useState } from "react";
import { Check, Copy, Edit3, FileText, Share2, X } from "lucide-react";
import type { I18n } from "../../lib/i18n";
import type { DetectedTool } from "../../types/skills";
import type { HealthCheckResult, McpServer } from "./helpers";

const CodeEditor = lazy(() => import("../../components/CodeEditor"));

type DetailTab = "overview" | "config" | "sync";

interface McpServerDetailPanelProps {
  selected: McpServer;
  i: I18n;
  zh: boolean;
  copied: boolean;
  copyConfig: () => void;
  startEdit: (server: McpServer) => void;
  saveSuccess: boolean;
  healthResult?: HealthCheckResult;
  getSourceBadge: (source: string) => string;
  getSourceLabel: (source: string) => string;
  installedTools: DetectedTool[];
  toolSyncStatus: Record<string, boolean>;
  syncingTo: string | null;
  toggleToolSync: (toolId: string) => void;
  onClose: () => void;
}

export default function McpServerDetailPanel({
  selected,
  i,
  zh,
  copied,
  copyConfig,
  startEdit,
  saveSuccess,
  healthResult,
  getSourceBadge,
  getSourceLabel,
  installedTools,
  toolSyncStatus,
  syncingTo,
  toggleToolSync,
  onClose,
}: McpServerDetailPanelProps) {
  const [activeTab, setActiveTab] = useState<DetailTab>("overview");

  useEffect(() => {
    setActiveTab("overview");
  }, [selected.id]);

  const tabs: Array<{ id: DetailTab; label: string }> = [
    { id: "overview", label: zh ? "概览" : "Overview" },
    { id: "config", label: zh ? "配置" : "Configuration" },
    { id: "sync", label: zh ? "同步" : "Sync" },
  ];

  return (
    <div className="entity-detail">
      <header className="entity-detail-header">
        <div className="min-w-0">
          <div className="entity-detail-heading">
            <span
              className={`dot ${selected.status === "active" ? "dot-active" : selected.status === "error" ? "dot-error" : "dot-disabled"}`}
            />
            <h3 className="entity-detail-title">{selected.name}</h3>
            {selected.version && <span className="text-[10px] text-muted-foreground">v{selected.version}</span>}
          </div>
          <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
            <span className={`badge ${selected.status === "active" ? "badge-success" : "badge-muted"}`}>
              {selected.status === "active" ? i.mcp.active : i.mcp.disabled}
            </span>
            <span className="badge badge-muted">{selected.transport}</span>
            <span className={`badge ${getSourceBadge(selected.source)}`}>{getSourceLabel(selected.source)}</span>
          </div>
        </div>
        <div className="entity-detail-actions">
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={copyConfig}
            title={zh ? "复制配置" : "Copy configuration"}
            aria-label={zh ? "复制配置" : "Copy configuration"}
          >
            {copied ? <Check size={14} className="text-[var(--success)]" /> : <Copy size={14} />}
          </button>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => startEdit(selected)}
            title={i.mcp.editConfig}
            aria-label={i.mcp.editConfig}
          >
            <Edit3 size={14} />
          </button>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={onClose}
            title={zh ? "关闭详情" : "Close details"}
            aria-label={zh ? "关闭详情" : "Close details"}
          >
            <X size={14} />
          </button>
        </div>
      </header>

      <div className="entity-detail-tabs" role="tablist" aria-label={zh ? "服务详情" : "Server details"}>
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.id}
            className={`entity-detail-tab ${activeTab === tab.id ? "entity-detail-tab-active" : ""}`}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      <div className="entity-detail-scroll">
        {saveSuccess && (
          <div className="mb-3 flex items-center gap-2 rounded-md bg-[var(--success-subtle)] px-3 py-2 text-xs font-medium text-[var(--success)]">
            <Check size={14} />
            {zh ? "已保存到配置文件" : "Saved to config file"}
          </div>
        )}

        {activeTab === "overview" && (
          <div>
            {selected.config_path && (
              <section className="entity-detail-section">
                <span className="field-label">{i.mcp.configPath}</span>
                <div className="flex min-w-0 items-center gap-2 rounded-md bg-[var(--control-background)] px-2.5 py-2">
                  <FileText size={13} className="shrink-0 text-muted-foreground" />
                  <span className="truncate font-mono text-[11px] text-[var(--text-secondary)]">
                    {selected.config_path}
                  </span>
                </div>
              </section>
            )}

            <section className="entity-detail-section">
              <span className="field-label">{i.mcp.command}</span>
              <div className="code-block text-[12px]">{selected.command || i.common.na}</div>
            </section>

            <section className="entity-detail-section">
              <span className="field-label">{i.mcp.healthStatus}</span>
              {healthResult ? (
                <div className="flex flex-wrap gap-1.5">
                  <span
                    className={`badge ${healthResult.status === "healthy" ? "badge-success" : healthResult.status === "unhealthy" ? "badge-danger" : "badge-muted"}`}
                  >
                    {healthResult.status === "healthy"
                      ? i.mcp.healthy
                      : healthResult.status === "unhealthy"
                        ? i.mcp.unhealthy
                        : i.mcp.unknown}
                  </span>
                  <span className={`badge ${healthResult.command_exists ? "badge-success" : "badge-danger"}`}>
                    {i.mcp.commandExists}: {healthResult.command_exists ? "OK" : "--"}
                  </span>
                  {healthResult.latency_ms != null && (
                    <span className="badge badge-muted">
                      {i.mcp.latency}: {healthResult.latency_ms}ms
                    </span>
                  )}
                  {healthResult.error_message && (
                    <div className="code-block mt-2 w-full text-[11px] text-[var(--danger)]">
                      {healthResult.error_message}
                    </div>
                  )}
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">
                  {zh ? "运行健康检查后会在这里显示结果。" : "Run a health check to see results here."}
                </p>
              )}
            </section>
          </div>
        )}

        {activeTab === "config" && (
          <div>
            <section className="entity-detail-section">
              <span className="field-label">{i.mcp.arguments}</span>
              <CodeEditor value={formatJson(selected.args)} language="json" readOnly minHeight={150} maxHeight={280} />
            </section>
            <section className="entity-detail-section">
              <span className="field-label">{i.mcp.environment}</span>
              <CodeEditor
                value={formatEnvironment(selected.env)}
                language="json"
                readOnly
                minHeight={130}
                maxHeight={260}
              />
            </section>
          </div>
        )}

        {activeTab === "sync" && (
          <section>
            <div className="mb-3 flex items-center gap-1.5 text-xs font-medium text-[var(--text-secondary)]">
              <Share2 size={13} />
              {zh ? "同步到其他工具" : "Sync to other tools"}
            </div>
            {installedTools.length > 0 ? (
              <div className="overflow-hidden rounded-lg border border-[var(--border-subtle)]">
                {installedTools.map((tool) => {
                  const isSynced = toolSyncStatus[tool.id] || false;
                  return (
                    <div
                      key={tool.id}
                      className="flex min-h-11 items-center justify-between gap-3 border-b border-[var(--border-subtle)] px-3 last:border-b-0"
                    >
                      <div className="min-w-0">
                        <div className="truncate text-xs font-medium">{tool.name}</div>
                        <div className="mt-0.5 text-[10px] text-muted-foreground">
                          {isSynced ? (zh ? "已同步" : "Synced") : zh ? "未同步" : "Not synced"}
                        </div>
                      </div>
                      <button
                        className={`btn btn-xs ${isSynced ? "btn-secondary" : "btn-primary"}`}
                        disabled={syncingTo === tool.id}
                        onClick={() => toggleToolSync(tool.id)}
                      >
                        {syncingTo === tool.id ? (
                          <div className="spinner size-[11px]" />
                        ) : isSynced ? (
                          <Check size={11} />
                        ) : (
                          <Share2 size={11} />
                        )}
                        {isSynced ? (zh ? "取消" : "Remove") : zh ? "同步" : "Sync"}
                      </button>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="entity-detail-empty">
                {zh ? "没有可同步的已安装工具" : "No installed tools available"}
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}

function formatEnvironment(value: string) {
  try {
    const environment = JSON.parse(value);
    return Object.keys(environment).length ? JSON.stringify(environment, null, 2) : "{}";
  } catch {
    return value;
  }
}

function formatJson(value: string) {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}
