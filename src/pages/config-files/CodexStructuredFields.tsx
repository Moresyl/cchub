import { useState } from "react";
import { Eye, EyeOff, RefreshCw } from "lucide-react";
import { Button } from "../../components/ui/button";
import { CheckboxField } from "../../components/ui/checkbox-field";
import { Input } from "../../components/ui/input";
import { SimpleSelect } from "../../components/ui/simple-select";
import ReasoningEffortSelect from "../../components/ReasoningEffortSelect";
import type { CodexStructuredConfig, CodexStructuredValidation } from "../../lib/codexConfig";

interface Props {
  zh: boolean;
  config: CodexStructuredConfig;
  validation: CodexStructuredValidation | null;
  onPatch: (patch: Partial<CodexStructuredConfig>) => void;
  onAddMcp: () => void;
  onContextWindow1M: (enabled: boolean) => void;
  apiKey: string;
  onApiKeyChange: (key: string) => void;
  invalidContextWindow: boolean;
  invalidCompactLimit: boolean;
}

export default function CodexStructuredFields({
  zh,
  config: codexStructuredConfig,
  validation: codexValidation,
  onPatch: updateCodexConfig,
  onAddMcp: repairCodexConfig,
  onContextWindow1M: toggleCodexContextWindow1M,
  apiKey: codexApiKey,
  onApiKeyChange: setCodexApiKey,
  invalidContextWindow,
  invalidCompactLimit,
}: Props) {
  const [showCodexApiKey, setShowCodexApiKey] = useState(false);
  return (
    <div className="section-card" style={{ padding: 16 }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
          marginBottom: 14,
          flexWrap: "wrap",
        }}
      >
        <div>
          <div style={{ fontSize: 14, fontWeight: 700 }}>{zh ? "Codex 结构化编辑" : "Codex Structured Editor"}</div>
          <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 4 }}>
            {zh
              ? "字段级编辑会直接同步到下方的 config.toml，原始内容仍可继续手动修改。"
              : "Field edits write back into the TOML below, while preserving raw editing for advanced cases."}
          </div>
        </div>
        <Button
          variant="secondary"
          onClick={repairCodexConfig}
          disabled={codexStructuredConfig.malformedMcpServers || codexStructuredConfig.mcpServers.length > 0}
          style={{ gap: 6 }}
        >
          <RefreshCw size={14} />
          {zh ? "添加 MCP 表" : "Add MCP Table"}
        </Button>
      </div>

      {codexValidation && (codexValidation.errors.length > 0 || codexValidation.warnings.length > 0) && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8, marginBottom: 14 }}>
          {codexValidation.errors.map((message) => (
            <div
              key={`error:${message}`}
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
          {codexValidation.warnings.map((message) => (
            <div
              key={`warning:${message}`}
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
        <div>
          <label className="field-label" htmlFor="config-provider">
            {zh ? "模型 Provider" : "Model Provider"}
          </label>
          <Input
            id="config-provider"
            value={codexStructuredConfig.modelProvider}
            onChange={(event) => updateCodexConfig({ modelProvider: event.target.value })}
            placeholder="custom"
          />
        </div>
        <div>
          <label className="field-label" htmlFor="config-provider-label">
            {zh ? "Provider 显示名" : "Provider Label"}
          </label>
          <Input
            id="config-provider-label"
            value={codexStructuredConfig.providerLabel}
            onChange={(event) => updateCodexConfig({ providerLabel: event.target.value })}
            placeholder="custom"
          />
        </div>
        <div>
          <label className="field-label" htmlFor="config-model">
            {zh ? "模型 ID" : "Model ID"}
          </label>
          <Input
            id="config-model"
            value={codexStructuredConfig.model}
            onChange={(event) => updateCodexConfig({ model: event.target.value })}
            placeholder="gpt-5.6-sol"
          />
        </div>
        <div>
          <label className="field-label" htmlFor="config-base-url">
            Base URL
          </label>
          <Input
            id="config-base-url"
            value={codexStructuredConfig.baseUrl}
            onChange={(event) => updateCodexConfig({ baseUrl: event.target.value })}
            placeholder="https://api.example.com/v1"
          />
        </div>
        <div>
          <label className="field-label" htmlFor="config-file-api-key">
            API Key
          </label>
          <div style={{ position: "relative" }}>
            <Input
              id="config-file-api-key"
              type={showCodexApiKey ? "text" : "password"}
              value={codexApiKey}
              onChange={(event) => setCodexApiKey(event.target.value)}
              placeholder="sk-..."
              style={{ paddingRight: 40 }}
            />
            <Button
              variant="ghost"
              size="icon"
              aria-label={
                showCodexApiKey ? (zh ? "隐藏 API Key" : "Hide API key") : zh ? "显示 API Key" : "Show API key"
              }
              type="button"
              onClick={() => setShowCodexApiKey((current) => !current)}
              style={{ position: "absolute", right: 4, top: "50%", transform: "translateY(-50%)" }}
            >
              {showCodexApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </Button>
          </div>
        </div>
        <div>
          <label className="field-label" htmlFor="config-file-reasoning-effort">
            {zh ? "推理强度" : "Reasoning Effort"}
          </label>
          <ReasoningEffortSelect
            id="config-file-reasoning-effort"
            value={codexStructuredConfig.reasoningEffort}
            localeText={(chinese, english) => (zh ? chinese : english)}
            onValueChange={(value) => updateCodexConfig({ reasoningEffort: value })}
          />
        </div>
        <div>
          <label className="field-label">{zh ? "Wire API" : "Wire API"}</label>
          <SimpleSelect
            value={codexStructuredConfig.wireApi}
            ariaLabel={zh ? "Wire API" : "Wire API"}
            options={["responses", "chat"].map((option) => ({ value: option, label: option }))}
            onValueChange={(value) => updateCodexConfig({ wireApi: value })}
          />
        </div>
        <div>
          <label className="field-label">{zh ? "执行人格" : "Personality"}</label>
          <SimpleSelect
            value={codexStructuredConfig.personality}
            ariaLabel={zh ? "执行人格" : "Personality"}
            options={["pragmatic", "full-auto", "auto-edit", "explain"].map((option) => ({
              value: option,
              label: option,
            }))}
            onValueChange={(value) => updateCodexConfig({ personality: value })}
          />
        </div>
        <div>
          <label className="field-label" htmlFor="config-context-window">
            {zh ? "上下文窗口" : "Context Window"}
          </label>
          <Input
            id="config-context-window"
            inputMode="numeric"
            aria-invalid={invalidContextWindow}
            value={codexStructuredConfig.modelContextWindow}
            onChange={(event) => updateCodexConfig({ modelContextWindow: event.target.value })}
            placeholder="1000000"
          />
        </div>
        <div>
          <label className="field-label" htmlFor="config-compact-limit">
            {zh ? "自动压缩阈值" : "Auto Compact Limit"}
          </label>
          <Input
            id="config-compact-limit"
            inputMode="numeric"
            aria-invalid={invalidCompactLimit}
            value={codexStructuredConfig.modelAutoCompactTokenLimit}
            onChange={(event) => updateCodexConfig({ modelAutoCompactTokenLimit: event.target.value })}
            placeholder="900000"
          />
        </div>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 18, marginTop: 14, flexWrap: "wrap" }}>
        <CheckboxField
          checked={codexStructuredConfig.disableResponseStorage}
          onCheckedChange={(checked) => updateCodexConfig({ disableResponseStorage: checked })}
          label={zh ? "禁用响应存储" : "Disable Response Storage"}
        />
        <CheckboxField
          checked={codexStructuredConfig.modelContextWindow === "1000000"}
          onCheckedChange={toggleCodexContextWindow1M}
          label={zh ? "1M 上下文窗口" : "1M Context Window"}
        />
        <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
          {codexStructuredConfig.mcpServers.length > 0
            ? zh
              ? `已检测到 ${codexStructuredConfig.mcpServers.length} 个 MCP Server: ${codexStructuredConfig.mcpServers.join(", ")}`
              : `${codexStructuredConfig.mcpServers.length} MCP server(s): ${codexStructuredConfig.mcpServers.join(", ")}`
            : zh
              ? "当前未配置 MCP Server 表项。"
              : "No MCP server sections detected yet."}
        </div>
      </div>
    </div>
  );
}
