import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, RefreshCw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
  type StructuredDraftFields,
} from "../lib/configProfiles";

type HermesFieldKey = "baseUrl" | "apiKey" | "model" | "hermesProvider" | "hermesApiKeyEnv";

function HermesConfigSectionComponent() {
  const locale = getLocale();
  const uiText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);
  const [draft, setDraft] = useState<StructuredDraftFields>(() => createDefaultStructuredFields("hermes"));
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [rootOverride, setRootOverride] = useState<string | null>(null);

  const updateDraft = useCallback((field: HermesFieldKey, value: string) => {
    setDraft((current) => ({ ...current, [field]: value }));
  }, []);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const [content, override] = await Promise.all([
        invoke<string>("read_tool_config", { toolId: "hermes" }),
        invoke<string | null>("get_hermes_root_override"),
      ]);
      setDraft(parseStructuredConfig("hermes", content));
      setRootOverride(override);
    } catch (error) {
      setDraft(createDefaultStructuredFields("hermes"));
      showToast("error", String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  const saveConfig = useCallback(async () => {
    setSaving(true);
    try {
      await invoke("write_tool_config", {
        toolId: "hermes",
        content: buildStructuredConfig("hermes", draft),
      });
      showToast(
        "success",
        uiText(
          "Hermes 配置已保存。首次写入会自动备份原 config.yaml，YAML 注释会丢失。",
          "Hermes config saved. The first write creates a backup of config.yaml and YAML comments will be lost.",
        ),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setSaving(false);
    }
  }, [draft, uiText]);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const preview = useMemo(() => buildStructuredConfig("hermes", draft), [draft]);

  return (
    <div className="card" style={{ padding: "16px 18px", marginBottom: 16 }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 14, flexWrap: "wrap" }}>
        <div>
          <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("Hermes 配置面板", "Hermes Config Panel")}</h4>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
            {uiText(
              "直接编辑 `config.yaml + .env` 的合并视图。Windows 请把根目录覆盖指向 WSL2 内的 `~/.hermes`。",
              "Edit the merged `config.yaml + .env` view directly. On Windows, point the root override to the WSL2 `~/.hermes` path.",
            )}
          </p>
          {rootOverride && (
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
              Root Override: {rootOverride}
            </p>
          )}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={() => void loadConfig()} disabled={loading} style={{ gap: 6 }}>
            {loading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
            {uiText("重新读取", "Reload")}
          </button>
          <button className="btn btn-primary btn-sm" onClick={() => void saveConfig()} disabled={saving} style={{ gap: 6 }}>
            {saving ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={14} />}
            {uiText("保存配置", "Save Config")}
          </button>
        </div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 14 }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <label className="field-label">Provider</label>
          <select className="input" value={draft.hermesProvider} onChange={(event) => updateDraft("hermesProvider", event.target.value)}>
            {["nous", "openrouter", "gemini", "zai", "kimi-coding", "anthropic", "custom"].map((option) => (
              <option key={option} value={option}>{option}</option>
            ))}
          </select>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <label className="field-label">API Key Env</label>
          <input className="input" value={draft.hermesApiKeyEnv} onChange={(event) => updateDraft("hermesApiKeyEnv", event.target.value)} placeholder="OPENROUTER_API_KEY" />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <label className="field-label">Base URL</label>
          <input className="input" value={draft.baseUrl} onChange={(event) => updateDraft("baseUrl", event.target.value)} placeholder="https://openrouter.ai/api/v1" />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <label className="field-label">Model</label>
          <input className="input" value={draft.model} onChange={(event) => updateDraft("model", event.target.value)} placeholder="anthropic/claude-sonnet-4.6" />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6, gridColumn: "1 / -1" }}>
          <label className="field-label">API Key</label>
          <input className="input" type="password" value={draft.apiKey} onChange={(event) => updateDraft("apiKey", event.target.value)} placeholder="sk-..." />
        </div>
      </div>

      <div style={{ marginTop: 16 }}>
        <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8, color: "var(--text-secondary)" }}>
          {uiText("生成后的快照预览", "Generated Snapshot Preview")}
        </div>
        <pre className="code-block" style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 260, overflow: "auto", fontSize: 11 }}>
          {preview}
        </pre>
      </div>
    </div>
  );
}

export default memo(HermesConfigSectionComponent);
