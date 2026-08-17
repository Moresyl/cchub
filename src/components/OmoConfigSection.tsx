/* eslint-disable @typescript-eslint/no-explicit-any */
import { memo, useCallback, useEffect, useMemo, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, RefreshCw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { buildStructuredConfig, parseStructuredConfig, supportsStructuredConfig } from "../lib/configProfiles";
import { showToast } from "./Toast";
import {
  OMO_BUILTIN_AGENTS,
  OMO_BUILTIN_CATEGORIES,
  OMO_SLIM_BUILTIN_AGENTS,
  type OmoAgentDef,
  type OmoCategoryDef,
} from "../types/omo";

interface OmoLocalConfigData {
  variant: string;
  filePath: string;
  lastModified: string | null;
  agents: Record<string, Record<string, unknown>>;
  categories: Record<string, Record<string, unknown>> | null;
  otherFields: Record<string, unknown>;
  pluginEnabled: boolean;
  plugins: string[];
  opencodeConfigPath: string;
}

interface ConfigProfile {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
}

interface VariantEditorState {
  loading: boolean;
  saving: boolean;
  data: OmoLocalConfigData | null;
  agents: Record<string, Record<string, unknown>>;
  categories: Record<string, Record<string, unknown>>;
  otherFieldsText: string;
}

interface OmoModelFieldProps {
  label: string;
  listId: string;
  value: string;
  placeholder: string;
  suggestedLabel: string;
  recommended?: string;
  onChange: (value: string) => void;
}

function createVariantState(): VariantEditorState {
  return {
    loading: true,
    saving: false,
    data: null,
    agents: {},
    categories: {},
    otherFieldsText: "",
  };
}

function cloneStore(store: Record<string, Record<string, unknown>>): Record<string, Record<string, unknown>> {
  return Object.fromEntries(Object.entries(store).map(([key, value]) => [key, { ...value }]));
}

function buildPreview(
  agents: Record<string, Record<string, unknown>>,
  categories: Record<string, Record<string, unknown>>,
  otherFieldsText: string,
  includeCategories: boolean,
) {
  const root: Record<string, unknown> = {};
  try {
    const parsed = otherFieldsText.trim() ? (JSON.parse(otherFieldsText) as Record<string, unknown>) : {};
    Object.assign(root, parsed);
  } catch {
    // ignore invalid preview input; the save action handles validation
  }
  if (Object.keys(agents).length > 0) {
    root.agents = agents;
  }
  if (includeCategories && Object.keys(categories).length > 0) {
    root.categories = categories;
  }
  return JSON.stringify(root, null, 2);
}

function extractModelSuggestionsFromRawConfig(content: string) {
  const suggestions = new Set<string>();

  try {
    const parsed = JSON.parse(content) as Record<string, any>;
    if (parsed.models && typeof parsed.models === "object") {
      Object.keys(parsed.models).forEach((key) => key.trim() && suggestions.add(key.trim()));
    }
  } catch {
    // ignore malformed local config
  }

  return suggestions;
}

function normalizeVariantData(data: OmoLocalConfigData): VariantEditorState {
  return {
    loading: false,
    saving: false,
    data,
    agents: cloneStore(data.agents || {}),
    categories: cloneStore(data.categories || {}),
    otherFieldsText: Object.keys(data.otherFields || {}).length ? JSON.stringify(data.otherFields, null, 2) : "",
  };
}

function OmoModelFieldComponent({
  label,
  listId,
  value,
  placeholder,
  suggestedLabel,
  recommended,
  onChange,
}: OmoModelFieldProps) {
  const handleChange = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      onChange(event.target.value);
    },
    [onChange],
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <label className="field-label">{label}</label>
      <input className="input" list={listId} value={value} onChange={handleChange} placeholder={placeholder} />
      {recommended ? (
        <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
          {suggestedLabel}: {recommended}
        </div>
      ) : null}
    </div>
  );
}

const OmoModelField = memo(OmoModelFieldComponent);

function VariantEditor({
  title,
  description,
  variant,
  state,
  onReload,
  onSave,
  onModelChange,
  onOtherFieldsTextChange,
  modelSuggestions,
  agentDefs,
  categoryDefs = [],
}: {
  title: string;
  description: string;
  variant: "standard" | "slim";
  state: VariantEditorState;
  onReload: () => void;
  onSave: () => void;
  onModelChange: (kind: "agents" | "categories", key: string, value: string) => void;
  onOtherFieldsTextChange: (value: string) => void;
  modelSuggestions: string[];
  agentDefs: OmoAgentDef[];
  categoryDefs?: OmoCategoryDef[];
}) {
  const locale = getLocale();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  const builtinAgentKeys = useMemo(() => new Set(agentDefs.map((item) => item.key)), [agentDefs]);
  const builtinCategoryKeys = useMemo(() => new Set(categoryDefs.map((item) => item.key)), [categoryDefs]);
  const customAgentKeys = Object.keys(state.agents).filter((key) => !builtinAgentKeys.has(key));
  const customCategoryKeys = Object.keys(state.categories).filter((key) => !builtinCategoryKeys.has(key));
  const preview = buildPreview(state.agents, state.categories, state.otherFieldsText, variant === "standard");
  const modelListId = `omo-model-list-${variant}`;
  const suggestedLabel = uiText("建议", "Suggested", "推奨");
  const otherFieldsPlaceholder = uiText(
    "可选。填写额外 JSON 对象字段，会原样并入 OMO 配置。",
    "Optional. Add any extra JSON object fields to merge into the OMO config.",
    "任意。追加の JSON オブジェクト項目を OMO 設定へそのままマージします。",
  );

  return (
    <div className="card" style={{ padding: "16px 18px", display: "flex", flexDirection: "column", gap: 14 }}>
      <div
        style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}
      >
        <div>
          <h4 style={{ fontSize: 13, fontWeight: 700 }}>{title}</h4>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4, lineHeight: 1.6 }}>{description}</p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={onReload}
            disabled={state.loading}
            style={{ gap: 6 }}
          >
            {state.loading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
            {uiText("重载", "Reload", "再読み込み")}
          </button>
          <button
            className="btn btn-primary btn-sm"
            type="button"
            onClick={onSave}
            disabled={state.loading || state.saving}
            style={{ gap: 6 }}
          >
            {state.saving ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={14} />}
            {uiText("保存到本地", "Save Local", "ローカル保存")}
          </button>
        </div>
      </div>

      {state.data ? (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))",
            gap: 10,
            fontSize: 11,
            color: "var(--text-muted)",
          }}
        >
          <div>
            {uiText("配置文件", "Config File", "設定ファイル")}: {state.data.filePath}
          </div>
          <div>
            {uiText("OpenCode 配置", "OpenCode Config", "OpenCode 設定")}: {state.data.opencodeConfigPath}
          </div>
          <div>
            {uiText("插件状态", "Plugin State", "プラグイン状態")}:{" "}
            {state.data.pluginEnabled ? uiText("已启用", "Enabled", "有効") : uiText("未启用", "Disabled", "無効")}
          </div>
          {state.data.lastModified ? (
            <div>
              {uiText("最后修改", "Last Modified", "最終更新")}: {state.data.lastModified}
            </div>
          ) : null}
        </div>
      ) : null}

      {customAgentKeys.length > 0 || customCategoryKeys.length > 0 ? (
        <div className="card" style={{ padding: 12, background: "var(--bg-elevated)" }}>
          <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.6 }}>
            {uiText(
              "面板只编辑内置 Agent / Category 的模型选择。文件里已有的自定义条目会在保存时继续保留。",
              "The panel edits built-in agent/category model bindings only. Existing custom entries in the file are preserved on save.",
              "このパネルでは組み込み Agent / Category のモデル紐付けのみ編集します。既存のカスタム項目は保存時に保持されます。",
            )}
          </div>
          <div style={{ marginTop: 8, display: "flex", gap: 8, flexWrap: "wrap" }}>
            {customAgentKeys.map((key) => (
              <span key={key} className="badge badge-muted">
                {key}
              </span>
            ))}
            {customCategoryKeys.map((key) => (
              <span key={key} className="badge badge-accent">
                {key}
              </span>
            ))}
          </div>
        </div>
      ) : null}

      <datalist id={modelListId}>
        {modelSuggestions.map((model) => (
          <option key={model} value={model} />
        ))}
      </datalist>

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <div>
          <div style={{ fontSize: 12, fontWeight: 700, marginBottom: 8 }}>
            {uiText("Agent 模型", "Agent Models", "Agent モデル")}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 12 }}>
            {agentDefs.map((agent) => (
              <OmoModelField
                key={agent.key}
                label={agent.display}
                listId={modelListId}
                value={(state.agents[agent.key]?.model as string) || ""}
                onChange={(value) => onModelChange("agents", agent.key, value)}
                placeholder={agent.recommended || uiText("输入模型 ID", "Enter model ID", "モデル ID を入力")}
                suggestedLabel={suggestedLabel}
                recommended={agent.recommended}
              />
            ))}
          </div>
        </div>

        {variant === "standard" && categoryDefs.length > 0 ? (
          <div>
            <div style={{ fontSize: 12, fontWeight: 700, marginBottom: 8 }}>
              {uiText("Category 模型", "Category Models", "Category モデル")}
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 12 }}>
              {categoryDefs.map((category) => (
                <OmoModelField
                  key={category.key}
                  label={category.display}
                  listId={modelListId}
                  value={(state.categories[category.key]?.model as string) || ""}
                  onChange={(value) => onModelChange("categories", category.key, value)}
                  placeholder={category.recommended || uiText("输入模型 ID", "Enter model ID", "モデル ID を入力")}
                  suggestedLabel={suggestedLabel}
                  recommended={category.recommended}
                />
              ))}
            </div>
          </div>
        ) : null}

        <div>
          <div style={{ fontSize: 12, fontWeight: 700, marginBottom: 8 }}>
            {uiText("保留其他字段", "Preserved Extra Fields", "保持する追加フィールド")}
          </div>
          <textarea
            className="input"
            value={state.otherFieldsText}
            onChange={(event) => onOtherFieldsTextChange(event.target.value)}
            placeholder={otherFieldsPlaceholder}
            style={{ minHeight: 120, resize: "vertical", fontFamily: "'JetBrains Mono', monospace" }}
          />
        </div>

        <div>
          <div style={{ fontSize: 12, fontWeight: 700, marginBottom: 8 }}>
            {uiText("生成预览", "Generated Preview", "生成プレビュー")}
          </div>
          <pre
            className="code-block"
            style={{ margin: 0, maxHeight: 280, overflow: "auto", whiteSpace: "pre-wrap", fontSize: 11 }}
          >
            {preview}
          </pre>
        </div>
      </div>
    </div>
  );
}

const MemoizedVariantEditor = memo(VariantEditor);

function OmoConfigSectionComponent() {
  const locale = getLocale();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  const [standardState, setStandardState] = useState<VariantEditorState>(() => createVariantState());
  const [slimState, setSlimState] = useState<VariantEditorState>(() => createVariantState());
  const [modelSuggestions, setModelSuggestions] = useState<string[]>([]);

  const loadModelSuggestions = useCallback(async () => {
    const suggestions = new Set<string>();

    try {
      const profiles = await invoke<ConfigProfile[]>("get_config_profiles");
      for (const profile of profiles) {
        if (!supportsStructuredConfig(profile.tool_id)) continue;
        const fields = parseStructuredConfig(profile.tool_id, profile.config_snapshot);
        [
          fields.model,
          fields.reasoningModel,
          fields.haikuModel,
          fields.sonnetModel,
          fields.opusModel,
          fields.suggestedPrimaryModel,
          ...fields.suggestedFallbackModels.split(",").map((item) => item.trim()),
        ]
          .filter(Boolean)
          .forEach((model) => suggestions.add(model));
      }
    } catch {
      // ignore profile-load failures
    }

    try {
      const current = await invoke<string>("read_tool_config", { toolId: "opencode" });
      extractModelSuggestionsFromRawConfig(current).forEach((model) => suggestions.add(model));
      const parsed = parseStructuredConfig("opencode", current);
      if (parsed.model.trim()) {
        suggestions.add(parsed.model.trim());
      }
    } catch {
      // ignore local config failures
    }

    setModelSuggestions([...suggestions].sort((left, right) => left.localeCompare(right)));

    // Runtime discovery can take a few seconds; keep configured suggestions responsive.
    void invoke<string[]>("get_opencode_runtime_models")
      .then((runtimeModels) => {
        runtimeModels.forEach((model) => model.trim() && suggestions.add(model.trim()));
        setModelSuggestions([...suggestions].sort((left, right) => left.localeCompare(right)));
      })
      .catch(() => undefined);
  }, []);

  const loadVariant = useCallback(
    async (variant: "standard" | "slim") => {
      if (variant === "standard") {
        setStandardState((current) => ({ ...current, loading: true }));
      } else {
        setSlimState((current) => ({ ...current, loading: true }));
      }

      try {
        const data = await invoke<OmoLocalConfigData>("omo_read_local_config", { variant });
        const normalized = normalizeVariantData(data);
        if (variant === "standard") {
          setStandardState(normalized);
        } else {
          setSlimState(normalized);
        }
      } catch (error) {
        const message = uiText(
          `读取 ${variant === "standard" ? "OMO" : "OMO Slim"} 配置失败: ${error}`,
          `Failed to load ${variant === "standard" ? "OMO" : "OMO Slim"} config: ${error}`,
          `${variant === "standard" ? "OMO" : "OMO Slim"} 設定の読み込みに失敗しました: ${error}`,
        );
        showToast("error", message);
        if (variant === "standard") {
          setStandardState((current) => ({ ...current, loading: false }));
        } else {
          setSlimState((current) => ({ ...current, loading: false }));
        }
      }
    },
    [uiText],
  );

  useEffect(() => {
    void Promise.all([loadVariant("standard"), loadVariant("slim"), loadModelSuggestions()]);
  }, [loadModelSuggestions, loadVariant]);

  const updateStoreModel = useCallback(
    (variant: "standard" | "slim", kind: "agents" | "categories", key: string, value: string) => {
      const setState = variant === "standard" ? setStandardState : setSlimState;
      setState((current) => {
        const nextStore = cloneStore(kind === "agents" ? current.agents : current.categories);
        const existing = { ...(nextStore[key] || {}) };
        const trimmed = value.trim();
        if (trimmed) {
          existing.model = trimmed;
          nextStore[key] = existing;
        } else {
          delete existing.model;
          if (Object.keys(existing).length > 0) {
            nextStore[key] = existing;
          } else {
            delete nextStore[key];
          }
        }

        return kind === "agents" ? { ...current, agents: nextStore } : { ...current, categories: nextStore };
      });
    },
    [],
  );

  const updateOtherFieldsText = useCallback((variant: "standard" | "slim", value: string) => {
    const setState = variant === "standard" ? setStandardState : setSlimState;
    setState((current) => ({ ...current, otherFieldsText: value }));
  }, []);

  const saveVariant = useCallback(
    async (variant: "standard" | "slim") => {
      const state = variant === "standard" ? standardState : slimState;
      const setState = variant === "standard" ? setStandardState : setSlimState;
      setState((current) => ({ ...current, saving: true }));

      try {
        const otherFields = state.otherFieldsText.trim() ? JSON.parse(state.otherFieldsText) : {};
        const saved = await invoke<OmoLocalConfigData>("omo_write_local_config", {
          variant,
          agents: state.agents,
          categories: variant === "standard" ? state.categories : null,
          otherFields,
        });
        setState(normalizeVariantData(saved));
        await loadModelSuggestions();
        showToast(
          "success",
          uiText(
            `${variant === "standard" ? "OMO" : "OMO Slim"} 配置已保存`,
            `${variant === "standard" ? "OMO" : "OMO Slim"} config saved`,
            `${variant === "standard" ? "OMO" : "OMO Slim"} 設定を保存しました`,
          ),
        );
      } catch (error) {
        showToast(
          "error",
          uiText(
            `保存 ${variant === "standard" ? "OMO" : "OMO Slim"} 失败: ${error}`,
            `Failed to save ${variant === "standard" ? "OMO" : "OMO Slim"}: ${error}`,
            `${variant === "standard" ? "OMO" : "OMO Slim"} の保存に失敗しました: ${error}`,
          ),
        );
        setState((current) => ({ ...current, saving: false }));
      }
    },
    [loadModelSuggestions, slimState, standardState, uiText],
  );

  const handleReloadStandard = useCallback(() => {
    void loadVariant("standard");
  }, [loadVariant]);

  const handleReloadSlim = useCallback(() => {
    void loadVariant("slim");
  }, [loadVariant]);

  const handleSaveStandard = useCallback(() => {
    void saveVariant("standard");
  }, [saveVariant]);

  const handleSaveSlim = useCallback(() => {
    void saveVariant("slim");
  }, [saveVariant]);

  const handleStandardModelChange = useCallback(
    (kind: "agents" | "categories", key: string, value: string) => {
      updateStoreModel("standard", kind, key, value);
    },
    [updateStoreModel],
  );

  const handleSlimModelChange = useCallback(
    (kind: "agents" | "categories", key: string, value: string) => {
      updateStoreModel("slim", kind, key, value);
    },
    [updateStoreModel],
  );

  const handleStandardOtherFieldsChange = useCallback(
    (value: string) => {
      updateOtherFieldsText("standard", value);
    },
    [updateOtherFieldsText],
  );

  const handleSlimOtherFieldsChange = useCallback(
    (value: string) => {
      updateOtherFieldsText("slim", value);
    },
    [updateOtherFieldsText],
  );

  const profilePreviewExample = useMemo(() => {
    return buildStructuredConfig("opencode", {
      ...parseStructuredConfig("opencode", "{}"),
      baseUrl: "",
      apiKey: "",
      model: "",
    });
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div className="card" style={{ padding: "16px 18px" }}>
        <div style={{ fontSize: 13, fontWeight: 700, marginBottom: 6 }}>
          {uiText("Oh My OpenCode (OMO)", "Oh My OpenCode (OMO)", "Oh My OpenCode (OMO)")}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.7 }}>
          {uiText(
            "这里直接管理本机 OpenCode 目录下的 OMO / OMO Slim 配置文件，并自动确保 `opencode.json` 中启用对应插件。面板只聚焦 Agent / Category 模型选择，其他 JSON 字段会原样保留。",
            "This edits the local OMO / OMO Slim config files in the OpenCode config directory and keeps the matching plugin enabled in `opencode.json`. The panel focuses on agent/category model selection while preserving the rest of the JSON.",
            "OpenCode 設定ディレクトリ内の OMO / OMO Slim 設定ファイルを直接編集し、`opencode.json` の対応プラグインも自動で有効化します。ここでは Agent / Category のモデル選択に集中し、その他の JSON 項目は保持します。",
          )}
        </div>
        {modelSuggestions.length > 0 ? (
          <div style={{ marginTop: 10, fontSize: 11, color: "var(--text-muted)" }}>
            {uiText(
              "可选模型会自动汇总自当前工具配置和已保存的 Provider Profiles。",
              "Model suggestions are aggregated from the current tool config and saved provider profiles.",
              "モデル候補は現在のツール設定と保存済み Provider Profiles から自動集約されます。",
            )}
          </div>
        ) : null}
        <details style={{ marginTop: 12 }}>
          <summary style={{ cursor: "pointer", fontSize: 12, color: "var(--text-secondary)" }}>
            {uiText(
              "当前 OpenCode 结构化配置参考",
              "Current OpenCode structured-config reference",
              "現在の OpenCode 構造化設定の参考",
            )}
          </summary>
          <pre
            className="code-block"
            style={{ marginTop: 10, maxHeight: 180, overflow: "auto", whiteSpace: "pre-wrap", fontSize: 11 }}
          >
            {profilePreviewExample}
          </pre>
        </details>
      </div>

      <MemoizedVariantEditor
        title={uiText("OMO 标准版", "OMO Standard", "OMO 標準版")}
        description={uiText(
          "管理 `oh-my-openagent.jsonc` / `oh-my-opencode.jsonc` 族配置，并提供 Agent + Category 的模型选择。",
          "Manages the `oh-my-openagent.jsonc` / `oh-my-opencode.jsonc` family with Agent and Category model selection.",
          "`oh-my-openagent.jsonc` / `oh-my-opencode.jsonc` 系の設定を管理し、Agent と Category のモデルを選択できます。",
        )}
        variant="standard"
        state={standardState}
        onReload={handleReloadStandard}
        onSave={handleSaveStandard}
        onModelChange={handleStandardModelChange}
        onOtherFieldsTextChange={handleStandardOtherFieldsChange}
        modelSuggestions={modelSuggestions}
        agentDefs={OMO_BUILTIN_AGENTS}
        categoryDefs={OMO_BUILTIN_CATEGORIES}
      />

      <MemoizedVariantEditor
        title={uiText("OMO Slim", "OMO Slim", "OMO Slim")}
        description={uiText(
          "管理 `oh-my-opencode-slim.jsonc` 轻量变体配置，适合更精简的多 Agent 组合。",
          "Manages the `oh-my-opencode-slim.jsonc` lightweight variant for a smaller multi-agent setup.",
          "`oh-my-opencode-slim.jsonc` の軽量版設定を管理し、より小さな multi-agent 構成に向いています。",
        )}
        variant="slim"
        state={slimState}
        onReload={handleReloadSlim}
        onSave={handleSaveSlim}
        onModelChange={handleSlimModelChange}
        onOtherFieldsTextChange={handleSlimOtherFieldsChange}
        modelSuggestions={modelSuggestions}
        agentDefs={OMO_SLIM_BUILTIN_AGENTS}
      />
    </div>
  );
}

export default memo(OmoConfigSectionComponent);
