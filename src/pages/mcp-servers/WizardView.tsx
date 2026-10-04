import { ArrowLeft, ArrowRight, PackagePlus, X } from "lucide-react";
import { useId } from "react";

import { WIZARD_PRESETS, type WizardPreset } from "./helpers";
import type { McpValidationResult, McpWizardDraft } from "../../hooks/useMcpValidation";
import type { I18n } from "../../lib/i18n";
import type { DetectedTool } from "../../types/skills";
import { CheckboxField } from "../../components/ui/checkbox-field";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "../../components/ui/dialog";
import { Textarea } from "../../components/ui/textarea";

import CodeEditor from "../../components/DeferredCodeEditor";

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
  const fieldId = useId();
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !wizardInstalling) closeWizard();
      }}
    >
      <DialogContent hideClose className="max-w-[760px] max-h-[min(680px,calc(100dvh-40px))]">
        <fieldset
          disabled={wizardInstalling}
          aria-busy={wizardInstalling}
          className="min-h-0 min-w-0 overflow-y-auto p-5"
        >
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
              <DialogTitle>{zh ? "MCP 安装向导" : "MCP Install Wizard"}</DialogTitle>
              <DialogDescription>
                {zh
                  ? "填写连接信息并选择同步目标，检查无误后一次性安装。"
                  : "Enter connection details, choose sync targets, and review before installing."}
              </DialogDescription>
            </div>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={i.common.close}
              title={i.common.close}
              onClick={closeWizard}
            >
              <X size={14} />
            </Button>
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
                  {zh ? "传输方式" : "Transport"}
                </div>
                <div role="group" aria-label={zh ? "传输方式" : "Transport"} style={{ display: "flex", gap: 6 }}>
                  {(["stdio", "http", "sse"] as const).map((transport) => (
                    <Button
                      key={transport}
                      type="button"
                      size="sm"
                      variant={wizardDraft.transport === transport ? "default" : "secondary"}
                      aria-pressed={wizardDraft.transport === transport}
                      onClick={() => setWizardDraft((current) => ({ ...current, transport }))}
                    >
                      {transport === "stdio" ? "STDIO" : transport.toUpperCase()}
                    </Button>
                  ))}
                </div>
              </div>

              {wizardDraft.transport === "stdio" && (
                <div>
                  <div className="field-label" style={{ marginBottom: 8 }}>
                    {zh ? "常用模板" : "Quick Templates"}
                  </div>
                  <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                    {WIZARD_PRESETS.map((preset) => (
                      <Button key={preset.id} variant="secondary" size="sm" onClick={() => applyWizardPreset(preset)}>
                        {zh ? preset.labelZh : preset.labelEn}
                      </Button>
                    ))}
                  </div>
                </div>
              )}

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 14 }}>
                <div>
                  <label className="field-label" htmlFor={`${fieldId}-name`}>
                    {zh ? "服务名称" : "Server Name"}
                  </label>
                  <Input
                    id={`${fieldId}-name`}
                    aria-label={zh ? "服务名称" : "Server Name"}
                    value={wizardDraft.name}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, name: event.target.value }))}
                    placeholder={zh ? "例如 filesystem" : "e.g. filesystem"}
                  />
                </div>
                <div>
                  <label className="field-label" htmlFor={`${fieldId}-command`}>
                    {wizardDraft.transport === "stdio" ? (zh ? "命令" : "Command") : zh ? "服务 URL" : "Server URL"}
                  </label>
                  <Input
                    id={`${fieldId}-command`}
                    aria-label={
                      wizardDraft.transport === "stdio" ? (zh ? "命令" : "Command") : zh ? "服务 URL" : "Server URL"
                    }
                    value={wizardDraft.command}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, command: event.target.value }))}
                    placeholder={
                      wizardDraft.transport === "stdio" ? "npx / uvx / docker / node" : "https://example.com/mcp"
                    }
                  />
                </div>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 14 }}>
                {wizardDraft.transport === "stdio" && (
                  <div>
                    <label className="field-label" htmlFor={`${fieldId}-args`}>
                      {zh ? "参数" : "Arguments"}
                    </label>
                    <Textarea
                      id={`${fieldId}-args`}
                      className="min-h-[118px] py-2.5 font-mono text-[12px]"
                      aria-label={zh ? "参数" : "Arguments"}
                      value={wizardDraft.argsText}
                      onChange={(event) => setWizardDraft((current) => ({ ...current, argsText: event.target.value }))}
                      placeholder={
                        zh ? "每行一个参数，或直接粘贴 JSON 数组" : "One argument per line, or paste a JSON array"
                      }
                    />
                  </div>
                )}
                <div>
                  <label className="field-label" htmlFor={`${fieldId}-env`}>
                    {wizardDraft.transport === "stdio" ? (zh ? "环境变量" : "Environment") : zh ? "请求头" : "Headers"}
                  </label>
                  <Textarea
                    id={`${fieldId}-env`}
                    className="min-h-[118px] py-2.5 font-mono text-[12px]"
                    aria-label={
                      wizardDraft.transport === "stdio" ? (zh ? "环境变量" : "Environment") : zh ? "请求头" : "Headers"
                    }
                    value={wizardDraft.envText}
                    onChange={(event) => setWizardDraft((current) => ({ ...current, envText: event.target.value }))}
                    placeholder={
                      zh
                        ? `每行 ${wizardDraft.transport === "stdio" ? "KEY=value" : "Header=value"}，或直接粘贴 JSON 对象`
                        : `Use ${wizardDraft.transport === "stdio" ? "KEY=value" : "Header=value"} per line, or paste a JSON object`
                    }
                  />
                </div>
              </div>
            </div>
          )}

          {wizardStep === 1 && wizardValidation.errors.length > 0 && (
            <div
              role="status"
              className="mt-4 space-y-1 rounded-md border border-border bg-muted/40 p-3 text-xs text-muted-foreground"
            >
              {wizardValidation.errors.map((message) => (
                <p key={message}>{message}</p>
              ))}
            </div>
          )}

          {wizardStep === 2 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              <div className="card" style={{ padding: 12, fontSize: 12, color: "var(--text-muted)" }}>
                {zh
                  ? "安装会默认写入 Claude 配置。下面可以额外勾选要同步到的其他工具。"
                  : "Install into Claude and optionally into the additional tools selected below."}
              </div>

              <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                <CheckboxField
                  checked
                  disabled
                  onCheckedChange={() => undefined}
                  label={
                    <>
                      Claude <span className="font-normal text-muted-foreground">{zh ? "默认" : "default"}</span>
                    </>
                  }
                />

                {wizardSyncableTools.length === 0 ? (
                  <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                    {zh ? "当前没有可同步的其他已安装工具。" : "No additional installed tools are available for sync."}
                  </div>
                ) : (
                  wizardSyncableTools.map((tool) => {
                    const checked = wizardSyncTargets.includes(tool.id);
                    return (
                      <CheckboxField
                        key={tool.id}
                        checked={checked}
                        onCheckedChange={() =>
                          setWizardSyncTargets((current) =>
                            checked ? current.filter((item) => item !== tool.id) : [...current, tool.id],
                          )
                        }
                        label={tool.name}
                      />
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
                  <div className="field-label">
                    {wizardDraft.transport === "stdio"
                      ? zh
                        ? "命令预览"
                        : "Command Preview"
                      : zh
                        ? "端点预览"
                        : "Endpoint Preview"}
                  </div>
                  <div className="code-block" style={{ fontSize: 12 }}>
                    {wizardDraft.transport === "stdio"
                      ? [wizardDraft.command.trim(), ...wizardValidation.parsedArgs].filter(Boolean).join(" ") ||
                        i.common.na
                      : wizardDraft.command.trim() || i.common.na}
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
                {wizardDraft.transport === "stdio" && (
                  <div>
                    <div className="field-label">{zh ? "参数解析结果" : "Parsed Arguments"}</div>
                    <CodeEditor
                      value={JSON.stringify(wizardValidation.parsedArgs, null, 2)}
                      language="json"
                      readOnly
                      minHeight={100}
                      maxHeight={180}
                    />
                  </div>
                )}
                <div>
                  <div className="field-label">
                    {wizardDraft.transport === "stdio"
                      ? zh
                        ? "环境变量解析结果"
                        : "Parsed Environment"
                      : zh
                        ? "请求头解析结果"
                        : "Parsed Headers"}
                  </div>
                  <CodeEditor
                    value={JSON.stringify(wizardValidation.parsedEnv, null, 2)}
                    language="json"
                    readOnly
                    minHeight={100}
                    maxHeight={180}
                  />
                </div>
              </div>
            </div>
          )}

          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 18, gap: 8, flexWrap: "wrap" }}>
            <div>
              {wizardStep > 1 && (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => setWizardStep((current) => Math.max(1, current - 1))}
                  style={{ gap: 6 }}
                >
                  <ArrowLeft size={14} />
                  {zh ? "上一步" : "Back"}
                </Button>
              )}
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <Button variant="secondary" size="sm" onClick={closeWizard}>
                {i.common.cancel}
              </Button>
              {wizardStep < 3 ? (
                <Button
                  size="sm"
                  onClick={() => setWizardStep((current) => Math.min(3, current + 1))}
                  disabled={wizardStep === 1 && !wizardValidation.isValid}
                  style={{ gap: 6 }}
                >
                  {zh ? "下一步" : "Next"}
                  <ArrowRight size={14} />
                </Button>
              ) : (
                <Button
                  size="sm"
                  onClick={() => void handleWizardInstall()}
                  disabled={!wizardValidation.isValid || wizardInstalling}
                  style={{ gap: 6 }}
                >
                  {wizardInstalling ? (
                    <div className="spinner" style={{ width: 12, height: 12 }} />
                  ) : (
                    <PackagePlus size={14} />
                  )}
                  {wizardInstalling ? (zh ? "安装中..." : "Installing...") : zh ? "确认安装" : "Install"}
                </Button>
              )}
            </div>
          </div>
        </fieldset>
      </DialogContent>
    </Dialog>
  );
}
