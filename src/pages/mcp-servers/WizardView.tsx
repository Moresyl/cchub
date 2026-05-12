import { lazy, Suspense } from "react";
import { ArrowLeft, ArrowRight, PackagePlus, X } from "lucide-react";

import { WIZARD_PRESETS, type WizardPreset } from "./helpers";
import type { McpValidationResult, McpWizardDraft } from "../../hooks/useMcpValidation";
import type { I18n } from "../../lib/i18n";
import type { DetectedTool } from "../../types/skills";

const CodeEditor = lazy(() => import("../../components/CodeEditor"));

interface McpServerWizardViewProps {
  zh: boolean;
  i: I18n;
  wizardStep: number;
  setWizardStep: React.Dispatch<React.SetStateAction<number>>;
  wizardDraft: McpWizardDraft;
  setWizardDraft: React.Dispatch<React.SetStateAction<McpWizardDraft>>;
  wizardSyncableTools: DetectedTool[];
  wizardSyncTargets: string[];
  setWizardSyncTargets: React.Dispatch<React.SetStateAction<string[]>>;
  wizardValidation: McpValidationResult;
  wizardInstalling: boolean;
  applyWizardPreset: (preset: WizardPreset) => void;
  closeWizard: () => void;
  handleWizardInstall: () => void;
}

export default function McpServerWizardView({
  zh,
  i,
  wizardStep,
  setWizardStep,
  wizardDraft,
  setWizardDraft,
  wizardSyncableTools,
  wizardSyncTargets,
  setWizardSyncTargets,
  wizardValidation,
  wizardInstalling,
  applyWizardPreset,
  closeWizard,
  handleWizardInstall,
}: McpServerWizardViewProps) {
  return (
    <div className="section-card" style={{ marginBottom: 16 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
          marginBottom: 16,
          flexWrap: "wrap",
        }}
      >
        <div>
          <div style={{ fontSize: 15, fontWeight: 700 }}>{zh ? "MCP 安装向导" : "MCP Install Wizard"}</div>
          <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
            {zh
              ? "按步骤填写命令、参数和环境变量，安装后会自动做一次健康检查。"
              : "Fill in command, arguments, and environment step by step. A health check will run automatically after install."}
          </div>
        </div>
        <button className="btn btn-ghost btn-icon-sm" onClick={closeWizard}>
          <X size={14} />
        </button>
      </div>

      <div style={{ display: "flex", gap: 8, marginBottom: 16, flexWrap: "wrap" }}>
        {[zh ? "1. 基本配置" : "1. Basics", zh ? "2. 同步目标" : "2. Sync", zh ? "3. 复核安装" : "3. Review"].map(
          (label, index) => (
            <div
              key={label}
              className={`badge ${wizardStep === index + 1 ? "badge-accent" : "badge-muted"}`}
              style={{ padding: "6px 10px" }}
            >
              {label}
            </div>
          ),
        )}
      </div>

      {wizardStep === 1 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div>
            <div className="field-label" style={{ marginBottom: 8 }}>
              {zh ? "常用模板" : "Quick Templates"}
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {WIZARD_PRESETS.map((preset) => (
                <button key={preset.id} className="btn btn-secondary btn-sm" onClick={() => applyWizardPreset(preset)}>
                  {zh ? preset.labelZh : preset.labelEn}
                </button>
              ))}
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 14 }}>
            <div>
              <label className="field-label">{zh ? "服务名称" : "Server Name"}</label>
              <input
                className="input"
                value={wizardDraft.name}
                onChange={(event) => setWizardDraft((current) => ({ ...current, name: event.target.value }))}
                placeholder={zh ? "例如 filesystem" : "e.g. filesystem"}
              />
            </div>
            <div>
              <label className="field-label">{zh ? "命令" : "Command"}</label>
              <input
                className="input"
                value={wizardDraft.command}
                onChange={(event) => setWizardDraft((current) => ({ ...current, command: event.target.value }))}
                placeholder="npx / uvx / docker / node"
              />
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 14 }}>
            <div>
              <label className="field-label">{zh ? "参数" : "Arguments"}</label>
              <textarea
                className="input"
                value={wizardDraft.argsText}
                onChange={(event) => setWizardDraft((current) => ({ ...current, argsText: event.target.value }))}
                placeholder={zh ? "每行一个参数，或直接粘贴 JSON 数组" : "One argument per line, or paste a JSON array"}
                style={{
                  minHeight: 118,
                  resize: "vertical",
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: 12,
                  paddingTop: 10,
                }}
              />
            </div>
            <div>
              <label className="field-label">{zh ? "环境变量" : "Environment"}</label>
              <textarea
                className="input"
                value={wizardDraft.envText}
                onChange={(event) => setWizardDraft((current) => ({ ...current, envText: event.target.value }))}
                placeholder={
                  zh ? "每行 KEY=value，或直接粘贴 JSON 对象" : "Use KEY=value per line, or paste a JSON object"
                }
                style={{
                  minHeight: 118,
                  resize: "vertical",
                  fontFamily: "'JetBrains Mono', monospace",
                  fontSize: 12,
                  paddingTop: 10,
                }}
              />
            </div>
          </div>
        </div>
      )}

      {wizardStep === 2 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          <div className="card" style={{ padding: 12, fontSize: 12, color: "var(--text-muted)" }}>
            {zh
              ? "安装会默认写入 Claude 配置。下面可以额外勾选要同步到的其他工具。"
              : "Install always writes to Claude first. Optionally sync the same server into other tools below."}
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, opacity: 0.8 }}>
              <input type="checkbox" checked readOnly />
              Claude
              <span style={{ color: "var(--text-muted)" }}>{zh ? "默认" : "default"}</span>
            </label>

            {wizardSyncableTools.length === 0 ? (
              <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                {zh ? "当前没有可同步的其他已安装工具。" : "No additional installed tools are available for sync."}
              </div>
            ) : (
              wizardSyncableTools.map((tool) => {
                const checked = wizardSyncTargets.includes(tool.id);
                return (
                  <label key={tool.id} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() =>
                        setWizardSyncTargets((current) =>
                          checked ? current.filter((item) => item !== tool.id) : [...current, tool.id],
                        )
                      }
                    />
                    {tool.name}
                  </label>
                );
              })
            )}
          </div>
        </div>
      )}

      {wizardStep === 3 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          {(wizardValidation.errors.length > 0 || wizardValidation.warnings.length > 0) && (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {wizardValidation.errors.map((message) => (
                <div
                  key={`wizard-error:${message}`}
                  className="card"
                  style={{
                    padding: "10px 12px",
                    borderColor: "var(--danger)",
                    background: "color-mix(in srgb, var(--danger) 8%, var(--bg-card))",
                    fontSize: 12,
                  }}
                >
                  {zh ? `错误：${message}` : `Error: ${message}`}
                </div>
              ))}
              {wizardValidation.warnings.map((message) => (
                <div
                  key={`wizard-warning:${message}`}
                  className="card"
                  style={{
                    padding: "10px 12px",
                    borderColor: "var(--warning)",
                    background: "color-mix(in srgb, var(--warning) 10%, var(--bg-card))",
                    fontSize: 12,
                  }}
                >
                  {zh ? `提示：${message}` : `Warning: ${message}`}
                </div>
              ))}
            </div>
          )}

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 14 }}>
            <div className="card" style={{ padding: 12 }}>
              <div className="field-label">{zh ? "命令预览" : "Command Preview"}</div>
              <div className="code-block" style={{ fontSize: 12 }}>
                {[wizardDraft.command.trim(), ...wizardValidation.parsedArgs].filter(Boolean).join(" ") || i.common.na}
              </div>
            </div>
            <div className="card" style={{ padding: 12 }}>
              <div className="field-label">{zh ? "同步到" : "Sync Targets"}</div>
              <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                {wizardSyncTargets.length > 0 ? `Claude, ${wizardSyncTargets.join(", ")}` : "Claude"}
              </div>
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 14 }}>
            <div>
              <div className="field-label">{zh ? "参数解析结果" : "Parsed Arguments"}</div>
              <Suspense fallback={null}>
                <CodeEditor
                  value={JSON.stringify(wizardValidation.parsedArgs, null, 2)}
                  language="json"
                  readOnly
                  minHeight={100}
                  maxHeight={180}
                />
              </Suspense>
            </div>
            <div>
              <div className="field-label">{zh ? "环境变量解析结果" : "Parsed Environment"}</div>
              <Suspense fallback={null}>
                <CodeEditor
                  value={JSON.stringify(wizardValidation.parsedEnv, null, 2)}
                  language="json"
                  readOnly
                  minHeight={100}
                  maxHeight={180}
                />
              </Suspense>
            </div>
          </div>
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 18, gap: 8, flexWrap: "wrap" }}>
        <div>
          {wizardStep > 1 && (
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => setWizardStep((current) => Math.max(1, current - 1))}
              style={{ gap: 6 }}
            >
              <ArrowLeft size={14} />
              {zh ? "上一步" : "Back"}
            </button>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={closeWizard}>
            {i.common.cancel}
          </button>
          {wizardStep < 3 ? (
            <button
              className="btn btn-primary btn-sm"
              onClick={() => setWizardStep((current) => Math.min(3, current + 1))}
              disabled={wizardStep === 1 && !wizardValidation.isValid}
              style={{ gap: 6 }}
            >
              {zh ? "下一步" : "Next"}
              <ArrowRight size={14} />
            </button>
          ) : (
            <button
              className="btn btn-primary btn-sm"
              onClick={() => void handleWizardInstall()}
              disabled={!wizardValidation.isValid || wizardInstalling}
              style={{ gap: 6 }}
            >
              {wizardInstalling ? (
                <div className="spinner" style={{ width: 12, height: 12 }} />
              ) : (
                <PackagePlus size={14} />
              )}
              {wizardInstalling ? (zh ? "安装中..." : "Installing...") : zh ? "安装并验证" : "Install & Verify"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
