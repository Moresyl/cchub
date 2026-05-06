import { memo, useCallback, useEffect, useMemo, useState, type ChangeEvent, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, RefreshCw, Search } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import EmptyState from "./states/EmptyState";
import LoadingState from "./states/LoadingState";
import {
  buildStructuredConfig,
  createDefaultStructuredFields,
  parseStructuredConfig,
  type OpenClawApiProtocol,
  type StructuredDraftFields,
} from "../lib/configProfiles";

interface OpenClawDailyMemoryEntry {
  path: string;
  file_name: string;
  source: string;
  project_name: string | null;
  modified_at: string | null;
  preview: string;
}

const OPENCLAW_PROTOCOL_OPTIONS: OpenClawApiProtocol[] = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "bedrock-converse-stream",
];

type OpenClawTextFieldKey =
  | "baseUrl"
  | "apiKey"
  | "modelCatalogAlias"
  | "model"
  | "modelName"
  | "openClawContextWindow"
  | "suggestedPrimaryModel"
  | "openClawCostInput"
  | "openClawCostOutput"
  | "suggestedFallbackModels";

interface OpenClawTextFieldProps {
  fieldKey: OpenClawTextFieldKey;
  label: string;
  value: string;
  placeholder?: string;
  type?: "text" | "password";
  onValueChange: (fieldKey: OpenClawTextFieldKey, value: string) => void;
}

interface OpenClawSelectFieldProps {
  label: string;
  value: OpenClawApiProtocol;
  options: OpenClawApiProtocol[];
  onValueChange: (value: OpenClawApiProtocol) => void;
}

interface OpenClawMemoryEntryCardProps {
  entry: OpenClawDailyMemoryEntry;
  active: boolean;
  globalLabel: string;
  projectLabel: string;
  onSelect: (entry: OpenClawDailyMemoryEntry) => void;
}

function OpenClawTextFieldComponent({
  fieldKey,
  label,
  value,
  placeholder,
  type = "text",
  onValueChange,
}: OpenClawTextFieldProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <label className="field-label">{label}</label>
      <input
        className="input"
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event: ChangeEvent<HTMLInputElement>) => onValueChange(fieldKey, event.target.value)}
      />
    </div>
  );
}

const OpenClawTextField = memo(OpenClawTextFieldComponent);

function OpenClawSelectFieldComponent({ label, value, options, onValueChange }: OpenClawSelectFieldProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <label className="field-label">{label}</label>
      <select
        className="input"
        value={value}
        onChange={(event: ChangeEvent<HTMLSelectElement>) => onValueChange(event.target.value as OpenClawApiProtocol)}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    </div>
  );
}

const OpenClawSelectField = memo(OpenClawSelectFieldComponent);

function OpenClawMemoryEntryCardComponent({
  entry,
  active,
  globalLabel,
  projectLabel,
  onSelect,
}: OpenClawMemoryEntryCardProps) {
  return (
    <button
      type="button"
      className="card"
      onClick={() => onSelect(entry)}
      style={{
        padding: "14px 16px",
        textAlign: "left",
        border: active ? "1px solid var(--accent-primary)" : "1px solid var(--border-color)",
        background: active
          ? "color-mix(in srgb, var(--accent-primary) 10%, var(--bg-secondary))"
          : "var(--bg-secondary)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start" }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 12, fontWeight: 700, color: "var(--text-primary)", wordBreak: "break-word" }}>
            {entry.file_name}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
            {entry.source === "global" ? globalLabel : projectLabel}
            {entry.project_name ? ` · ${entry.project_name}` : ""}
          </div>
        </div>
        {entry.modified_at ? (
          <span style={{ fontSize: 10, color: "var(--text-muted)", whiteSpace: "nowrap" }}>{entry.modified_at}</span>
        ) : null}
      </div>
      <div style={{ fontSize: 11, color: "var(--text-secondary)", lineHeight: 1.6 }}>{entry.preview}</div>
      <div style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>{entry.path}</div>
    </button>
  );
}

const OpenClawMemoryEntryCard = memo(OpenClawMemoryEntryCardComponent);

function OpenClawConfigSectionComponent() {
  const locale = getLocale();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  const [draft, setDraft] = useState<StructuredDraftFields>(() => createDefaultStructuredFields("openclaw"));
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [memoryQuery, setMemoryQuery] = useState("");
  const [memoryEntries, setMemoryEntries] = useState<OpenClawDailyMemoryEntry[]>([]);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [memorySelectedPath, setMemorySelectedPath] = useState<string | null>(null);
  const [memoryContent, setMemoryContent] = useState("");
  const [memoryLoadingContent, setMemoryLoadingContent] = useState(false);

  const updateDraft = useCallback((next: Partial<StructuredDraftFields>) => {
    setDraft((current) => ({ ...current, ...next }));
  }, []);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    try {
      const content = await invoke<string>("read_tool_config", { toolId: "openclaw" });
      setDraft(parseStructuredConfig("openclaw", content));
    } catch (e) {
      console.error(e);
      setDraft(createDefaultStructuredFields("openclaw"));
      showToast(
        "error",
        uiText(
          `读取 OpenClaw 配置失败: ${e}`,
          `Failed to load OpenClaw config: ${e}`,
          `OpenClaw 設定の読み込みに失敗しました: ${e}`,
        ),
      );
    } finally {
      setLoading(false);
    }
  }, [uiText]);

  const saveConfig = useCallback(async () => {
    setSaving(true);
    try {
      await invoke("write_tool_config", {
        toolId: "openclaw",
        content: buildStructuredConfig("openclaw", draft),
      });
      showToast("success", uiText("OpenClaw 配置已保存", "OpenClaw config saved", "OpenClaw 設定を保存しました"));
    } catch (e) {
      showToast(
        "error",
        uiText(
          `保存 OpenClaw 配置失败: ${e}`,
          `Failed to save OpenClaw config: ${e}`,
          `OpenClaw 設定の保存に失敗しました: ${e}`,
        ),
      );
    } finally {
      setSaving(false);
    }
  }, [draft, uiText]);

  const openMemoryEntry = useCallback(
    async (entry: OpenClawDailyMemoryEntry) => {
      setMemorySelectedPath(entry.path);
      setMemoryLoadingContent(true);
      try {
        const content = await invoke<string>("read_openclaw_daily_memory_content", { path: entry.path });
        setMemoryContent(content);
      } catch (e) {
        setMemoryContent("");
        showToast(
          "error",
          uiText(
            `读取 Daily Memory 失败: ${e}`,
            `Failed to load Daily Memory entry: ${e}`,
            `Daily Memory の読み込みに失敗しました: ${e}`,
          ),
        );
      } finally {
        setMemoryLoadingContent(false);
      }
    },
    [uiText],
  );

  const loadDailyMemory = useCallback(
    async (query = memoryQuery) => {
      setMemoryLoading(true);
      try {
        const entries = await invoke<OpenClawDailyMemoryEntry[]>("search_openclaw_daily_memory", { query, limit: 40 });
        setMemoryEntries(entries);
        const nextSelectedPath = entries.some((e) => e.path === memorySelectedPath)
          ? memorySelectedPath
          : (entries[0]?.path ?? null);
        const nextEntry = nextSelectedPath ? (entries.find((e) => e.path === nextSelectedPath) ?? null) : null;
        setMemorySelectedPath(nextSelectedPath);
        if (nextEntry) {
          await openMemoryEntry(nextEntry);
        } else {
          setMemoryContent("");
        }
      } catch (e) {
        setMemoryEntries([]);
        setMemorySelectedPath(null);
        setMemoryContent("");
        showToast(
          "error",
          uiText(
            `搜索 Daily Memory 失败: ${e}`,
            `Failed to search Daily Memory: ${e}`,
            `Daily Memory の検索に失敗しました: ${e}`,
          ),
        );
      } finally {
        setMemoryLoading(false);
      }
    },
    [memoryQuery, memorySelectedPath, openMemoryEntry, uiText],
  );

  useEffect(() => {
    void loadConfig();
    void loadDailyMemory();
  }, [loadConfig, loadDailyMemory]);

  const handleChangeDraftField = useCallback(
    (fieldKey: OpenClawTextFieldKey, value: string) => {
      updateDraft({ [fieldKey]: value } as Partial<StructuredDraftFields>);
    },
    [updateDraft],
  );

  const handleChangeApiProtocol = useCallback(
    (value: OpenClawApiProtocol) => {
      updateDraft({ apiProtocol: value });
    },
    [updateDraft],
  );

  const handleReloadConfigClick = useCallback(() => {
    void loadConfig();
  }, [loadConfig]);

  const handleSaveConfigClick = useCallback(() => {
    void saveConfig();
  }, [saveConfig]);

  const handleRefreshMemoryClick = useCallback(() => {
    void loadDailyMemory();
  }, [loadDailyMemory]);

  const handleChangeMemoryQuery = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setMemoryQuery(event.target.value);
  }, []);

  const handleMemoryQueryKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Enter") {
        event.preventDefault();
        void loadDailyMemory();
      }
    },
    [loadDailyMemory],
  );

  const handleSelectMemoryEntry = useCallback(
    (entry: OpenClawDailyMemoryEntry) => {
      void openMemoryEntry(entry);
    },
    [openMemoryEntry],
  );

  const configTextFields = useMemo(
    () => [
      {
        fieldKey: "baseUrl" as const,
        label: uiText("接口地址", "Base URL", "Base URL"),
        value: draft.baseUrl,
        placeholder: "https://api.example.com/v1",
      },
      {
        fieldKey: "apiKey" as const,
        label: "API Key",
        value: draft.apiKey,
        placeholder: uiText("填写 API Key", "Enter API Key", "API Key を入力"),
        type: "password" as const,
      },
      {
        fieldKey: "modelCatalogAlias" as const,
        label: uiText("模型别名", "Model Alias", "モデル別名"),
        value: draft.modelCatalogAlias,
        placeholder: "Claude Sonnet 4.6",
      },
      {
        fieldKey: "model" as const,
        label: uiText("模型 ID", "Model ID", "モデル ID"),
        value: draft.model,
        placeholder: "anthropic/claude-sonnet-4-6",
      },
      {
        fieldKey: "modelName" as const,
        label: uiText("显示名", "Display Name", "表示名"),
        value: draft.modelName,
        placeholder: uiText(
          "可选，默认同模型 ID",
          "Optional, defaults to model ID",
          "任意。未入力ならモデル ID を使います",
        ),
      },
      {
        fieldKey: "openClawContextWindow" as const,
        label: uiText("上下文窗口", "Context Window", "コンテキストウィンドウ"),
        value: draft.openClawContextWindow,
        placeholder: "1000000",
      },
      {
        fieldKey: "suggestedPrimaryModel" as const,
        label: uiText("主推荐模型", "Suggested Primary", "推奨プライマリ"),
        value: draft.suggestedPrimaryModel,
        placeholder: "anthropic/claude-sonnet-4-6",
      },
      {
        fieldKey: "openClawCostInput" as const,
        label: uiText("输入成本", "Input Cost", "入力コスト"),
        value: draft.openClawCostInput,
        placeholder: "0.003",
      },
      {
        fieldKey: "openClawCostOutput" as const,
        label: uiText("输出成本", "Output Cost", "出力コスト"),
        value: draft.openClawCostOutput,
        placeholder: "0.015",
      },
      {
        fieldKey: "suggestedFallbackModels" as const,
        label: uiText("备用模型", "Fallback Models", "フォールバックモデル"),
        value: draft.suggestedFallbackModels,
        placeholder: uiText(
          "逗号分隔，例如 model-a, model-b",
          "Comma-separated, e.g. model-a, model-b",
          "カンマ区切り。例: model-a, model-b",
        ),
      },
    ],
    [draft, uiText],
  );

  const globalMemoryLabel = uiText("全局", "Global", "グローバル");
  const projectMemoryLabel = uiText("项目", "Project", "プロジェクト");

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {/* Config Panel */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            gap: 12,
            alignItems: "center",
            marginBottom: 14,
            flexWrap: "wrap",
          }}
        >
          <div>
            <h4 style={{ fontSize: 13, fontWeight: 700 }}>
              {uiText("OpenClaw 配置面板", "OpenClaw Config Panel", "OpenClaw 設定パネル")}
            </h4>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
              {uiText(
                "直接编辑 OpenClaw 的 Provider / 模型 / Agent 建议参数，并同步写回 `~/.openclaw/openclaw.json`。",
                "Edit OpenClaw provider, model, and agent defaults, then write back to `~/.openclaw/openclaw.json`.",
                "`~/.openclaw/openclaw.json` に OpenClaw の Provider・モデル・Agent 既定値を書き戻します。",
              )}
            </p>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              className="btn btn-secondary btn-sm"
              onClick={handleReloadConfigClick}
              disabled={loading}
              style={{ gap: 6 }}
            >
              {loading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
              {uiText("重新读取", "Reload", "再読み込み")}
            </button>
            <button
              className="btn btn-primary btn-sm"
              onClick={handleSaveConfigClick}
              disabled={saving}
              style={{ gap: 6 }}
            >
              {saving ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={14} />}
              {uiText("保存配置", "Save Config", "設定を保存")}
            </button>
          </div>
        </div>

        {loading ? (
          <LoadingState
            label={uiText("正在读取 OpenClaw 配置...", "Loading OpenClaw config...", "OpenClaw 設定を読み込み中...")}
          />
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 14 }}>
              {configTextFields.slice(0, 2).map((field) => (
                <OpenClawTextField
                  key={field.fieldKey}
                  fieldKey={field.fieldKey}
                  label={field.label}
                  value={field.value}
                  placeholder={field.placeholder}
                  type={field.type}
                  onValueChange={handleChangeDraftField}
                />
              ))}
              <OpenClawSelectField
                label={uiText("API 协议", "API Protocol", "API プロトコル")}
                value={draft.apiProtocol}
                options={OPENCLAW_PROTOCOL_OPTIONS}
                onValueChange={handleChangeApiProtocol}
              />
              {configTextFields.slice(2, 9).map((field) => (
                <OpenClawTextField
                  key={field.fieldKey}
                  fieldKey={field.fieldKey}
                  label={field.label}
                  value={field.value}
                  placeholder={field.placeholder}
                  type={field.type}
                  onValueChange={handleChangeDraftField}
                />
              ))}
            </div>
            <OpenClawTextField
              fieldKey={configTextFields[9].fieldKey}
              label={configTextFields[9].label}
              value={configTextFields[9].value}
              placeholder={configTextFields[9].placeholder}
              type={configTextFields[9].type}
              onValueChange={handleChangeDraftField}
            />
            <div>
              <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8, color: "var(--text-secondary)" }}>
                {uiText("生成后的配置预览", "Generated Config Preview", "生成された設定プレビュー")}
              </div>
              <pre
                className="code-block"
                style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 260, overflow: "auto", fontSize: 11 }}
              >
                {buildStructuredConfig("openclaw", draft)}
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* Daily Memory */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            gap: 12,
            alignItems: "center",
            marginBottom: 14,
            flexWrap: "wrap",
          }}
        >
          <div>
            <h4 style={{ fontSize: 13, fontWeight: 700 }}>OpenClaw Daily Memory</h4>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
              {uiText(
                "搜索 `~/.openclaw` 与已发现项目下 `.openclaw` 目录中的 Daily Memory / Journal / Diary 文本。",
                "Search Daily Memory, Journal, and Diary text files in `~/.openclaw` and discovered project `.openclaw` directories.",
                "`~/.openclaw` と検出済みプロジェクトの `.openclaw` 配下にある Daily Memory / Journal / Diary テキストを検索します。",
              )}
            </p>
          </div>
          <button
            className="btn btn-secondary btn-sm"
            onClick={handleRefreshMemoryClick}
            disabled={memoryLoading}
            style={{ gap: 6 }}
          >
            {memoryLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
            {uiText("刷新结果", "Refresh", "再読み込み")}
          </button>
        </div>

        <div style={{ display: "flex", gap: 10, marginBottom: 14, flexWrap: "wrap" }}>
          <input
            className="input"
            value={memoryQuery}
            onChange={handleChangeMemoryQuery}
            onKeyDown={handleMemoryQueryKeyDown}
            placeholder={uiText(
              "输入关键词，留空则显示最近记录",
              "Enter a keyword, or leave empty for recent entries",
              "キーワードを入力。空欄なら最近の記録を表示",
            )}
            style={{ flex: "1 1 280px" }}
          />
          <button
            className="btn btn-primary btn-sm"
            onClick={handleRefreshMemoryClick}
            disabled={memoryLoading}
            style={{ gap: 6 }}
          >
            {memoryLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Search size={14} />}
            {memoryQuery.trim() ? uiText("搜索", "Search", "検索") : uiText("最近记录", "Recent Entries", "最近の記録")}
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: 14 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {memoryLoading && memoryEntries.length === 0 ? (
              <LoadingState
                label={uiText("正在搜索 Daily Memory...", "Searching Daily Memory...", "Daily Memory を検索中...")}
              />
            ) : memoryEntries.length === 0 ? (
              <EmptyState
                title={uiText("没有匹配结果", "No matching entries", "一致する結果はありません")}
                description={uiText(
                  "会扫描全局 `~/.openclaw` 与已发现项目目录中的 Daily Memory / Journal / Diary 文件。",
                  "The search scans global `~/.openclaw` and discovered project Daily Memory / Journal / Diary files.",
                  "グローバル `~/.openclaw` と検出済みプロジェクト内の Daily Memory / Journal / Diary を走査します。",
                )}
              />
            ) : (
              memoryEntries.map((entry) => (
                <OpenClawMemoryEntryCard
                  key={entry.path}
                  entry={entry}
                  active={entry.path === memorySelectedPath}
                  globalLabel={globalMemoryLabel}
                  projectLabel={projectMemoryLabel}
                  onSelect={handleSelectMemoryEntry}
                />
              ))
            )}
          </div>

          <div className="card" style={{ padding: "14px 16px", minHeight: 280, background: "var(--bg-secondary)" }}>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                gap: 10,
                alignItems: "center",
                marginBottom: 10,
                flexWrap: "wrap",
              }}
            >
              <div style={{ fontSize: 12, fontWeight: 700 }}>
                {uiText("全文预览", "Full Content", "全文プレビュー")}
              </div>
              {memorySelectedPath && (
                <div style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>
                  {memorySelectedPath}
                </div>
              )}
            </div>
            {memoryLoadingContent ? (
              <LoadingState label={uiText("正在读取全文...", "Loading full content...", "全文を読み込み中...")} />
            ) : memoryContent ? (
              <pre
                className="code-block"
                style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 420, overflow: "auto", fontSize: 11 }}
              >
                {memoryContent}
              </pre>
            ) : (
              <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.7 }}>
                {uiText(
                  "选择左侧结果以查看 Daily Memory 全文。",
                  "Select an entry on the left to inspect the full Daily Memory content.",
                  "左側の結果を選択すると Daily Memory の全文を表示します。",
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default memo(OpenClawConfigSectionComponent);
