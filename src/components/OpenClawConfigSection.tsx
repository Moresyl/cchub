import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Download, RefreshCw, Search } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
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

export default function OpenClawConfigSection() {
  const locale = getLocale();
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
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

  useEffect(() => {
    void loadConfig();
    void loadDailyMemory();
  }, []);

  function updateDraft(next: Partial<StructuredDraftFields>) {
    setDraft((current) => ({ ...current, ...next }));
  }

  async function loadConfig() {
    setLoading(true);
    try {
      const content = await invoke<string>("read_tool_config", { toolId: "openclaw" });
      setDraft(parseStructuredConfig("openclaw", content));
    } catch (e) {
      console.error(e);
      setDraft(createDefaultStructuredFields("openclaw"));
      showToast("error", uiText(`读取 OpenClaw 配置失败: ${e}`, `Failed to load OpenClaw config: ${e}`, `OpenClaw 設定の読み込みに失敗しました: ${e}`));
    } finally {
      setLoading(false);
    }
  }

  async function saveConfig() {
    setSaving(true);
    try {
      await invoke("write_tool_config", {
        toolId: "openclaw",
        content: buildStructuredConfig("openclaw", draft),
      });
      showToast("success", uiText("OpenClaw 配置已保存", "OpenClaw config saved", "OpenClaw 設定を保存しました"));
    } catch (e) {
      showToast("error", uiText(`保存 OpenClaw 配置失败: ${e}`, `Failed to save OpenClaw config: ${e}`, `OpenClaw 設定の保存に失敗しました: ${e}`));
    } finally {
      setSaving(false);
    }
  }

  async function openMemoryEntry(entry: OpenClawDailyMemoryEntry) {
    setMemorySelectedPath(entry.path);
    setMemoryLoadingContent(true);
    try {
      const content = await invoke<string>("read_openclaw_daily_memory_content", { path: entry.path });
      setMemoryContent(content);
    } catch (e) {
      setMemoryContent("");
      showToast("error", uiText(`读取 Daily Memory 失败: ${e}`, `Failed to load Daily Memory entry: ${e}`, `Daily Memory の読み込みに失敗しました: ${e}`));
    } finally {
      setMemoryLoadingContent(false);
    }
  }

  async function loadDailyMemory(query = memoryQuery) {
    setMemoryLoading(true);
    try {
      const entries = await invoke<OpenClawDailyMemoryEntry[]>("search_openclaw_daily_memory", { query, limit: 40 });
      setMemoryEntries(entries);
      const nextSelectedPath = entries.some((e) => e.path === memorySelectedPath) ? memorySelectedPath : (entries[0]?.path ?? null);
      const nextEntry = nextSelectedPath ? entries.find((e) => e.path === nextSelectedPath) ?? null : null;
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
      showToast("error", uiText(`搜索 Daily Memory 失败: ${e}`, `Failed to search Daily Memory: ${e}`, `Daily Memory の検索に失敗しました: ${e}`));
    } finally {
      setMemoryLoading(false);
    }
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      {/* Config Panel */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 14, flexWrap: "wrap" }}>
          <div>
            <h4 style={{ fontSize: 13, fontWeight: 700 }}>{uiText("OpenClaw 配置面板", "OpenClaw Config Panel", "OpenClaw 設定パネル")}</h4>
            <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
              {uiText(
                "直接编辑 OpenClaw 的 Provider / 模型 / Agent 建议参数，并同步写回 `~/.openclaw/openclaw.json`。",
                "Edit OpenClaw provider, model, and agent defaults, then write back to `~/.openclaw/openclaw.json`.",
                "`~/.openclaw/openclaw.json` に OpenClaw の Provider・モデル・Agent 既定値を書き戻します。",
              )}
            </p>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-secondary btn-sm" onClick={() => void loadConfig()} disabled={loading} style={{ gap: 6 }}>
              {loading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
              {uiText("重新读取", "Reload", "再読み込み")}
            </button>
            <button className="btn btn-primary btn-sm" onClick={() => void saveConfig()} disabled={saving} style={{ gap: 6 }}>
              {saving ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Download size={14} />}
              {uiText("保存配置", "Save Config", "設定を保存")}
            </button>
          </div>
        </div>

        {loading ? (
          <div className="loading-center" style={{ minHeight: 180 }}><div className="spinner" /></div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 14 }}>
              <FieldCard label={uiText("接口地址", "Base URL", "Base URL")} input={<input className="input" value={draft.baseUrl} onChange={(e) => updateDraft({ baseUrl: e.target.value })} placeholder="https://api.example.com/v1" />} />
              <FieldCard label="API Key" input={<input className="input" type="password" value={draft.apiKey} onChange={(e) => updateDraft({ apiKey: e.target.value })} placeholder={uiText("填写 API Key", "Enter API Key", "API Key を入力")} />} />
              <FieldCard
                label={uiText("API 协议", "API Protocol", "API プロトコル")}
                input={
                  <select className="input" value={draft.apiProtocol} onChange={(e) => updateDraft({ apiProtocol: e.target.value as OpenClawApiProtocol })}>
                    {OPENCLAW_PROTOCOL_OPTIONS.map((option) => <option key={option} value={option}>{option}</option>)}
                  </select>
                }
              />
              <FieldCard label={uiText("模型别名", "Model Alias", "モデル別名")} input={<input className="input" value={draft.modelCatalogAlias} onChange={(e) => updateDraft({ modelCatalogAlias: e.target.value })} placeholder="Claude Sonnet 4.6" />} />
              <FieldCard label={uiText("模型 ID", "Model ID", "モデル ID")} input={<input className="input" value={draft.model} onChange={(e) => updateDraft({ model: e.target.value })} placeholder="anthropic/claude-sonnet-4-6" />} />
              <FieldCard label={uiText("显示名", "Display Name", "表示名")} input={<input className="input" value={draft.modelName} onChange={(e) => updateDraft({ modelName: e.target.value })} placeholder={uiText("可选，默认同模型 ID", "Optional, defaults to model ID", "任意。未入力ならモデル ID を使います")} />} />
              <FieldCard label={uiText("上下文窗口", "Context Window", "コンテキストウィンドウ")} input={<input className="input" value={draft.openClawContextWindow} onChange={(e) => updateDraft({ openClawContextWindow: e.target.value })} placeholder="1000000" />} />
              <FieldCard label={uiText("主推荐模型", "Suggested Primary", "推奨プライマリ")} input={<input className="input" value={draft.suggestedPrimaryModel} onChange={(e) => updateDraft({ suggestedPrimaryModel: e.target.value })} placeholder="anthropic/claude-sonnet-4-6" />} />
              <FieldCard label={uiText("输入成本", "Input Cost", "入力コスト")} input={<input className="input" value={draft.openClawCostInput} onChange={(e) => updateDraft({ openClawCostInput: e.target.value })} placeholder="0.003" />} />
              <FieldCard label={uiText("输出成本", "Output Cost", "出力コスト")} input={<input className="input" value={draft.openClawCostOutput} onChange={(e) => updateDraft({ openClawCostOutput: e.target.value })} placeholder="0.015" />} />
            </div>
            <FieldCard label={uiText("备用模型", "Fallback Models", "フォールバックモデル")} input={<input className="input" value={draft.suggestedFallbackModels} onChange={(e) => updateDraft({ suggestedFallbackModels: e.target.value })} placeholder={uiText("逗号分隔，例如 model-a, model-b", "Comma-separated, e.g. model-a, model-b", "カンマ区切り。例: model-a, model-b")} />} />
            <div>
              <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8, color: "var(--text-secondary)" }}>
                {uiText("生成后的配置预览", "Generated Config Preview", "生成された設定プレビュー")}
              </div>
              <pre className="code-block" style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 260, overflow: "auto", fontSize: 11 }}>
                {buildStructuredConfig("openclaw", draft)}
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* Daily Memory */}
      <div className="card" style={{ padding: "16px 18px" }}>
        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 14, flexWrap: "wrap" }}>
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
          <button className="btn btn-secondary btn-sm" onClick={() => void loadDailyMemory()} disabled={memoryLoading} style={{ gap: 6 }}>
            {memoryLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
            {uiText("刷新结果", "Refresh", "再読み込み")}
          </button>
        </div>

        <div style={{ display: "flex", gap: 10, marginBottom: 14, flexWrap: "wrap" }}>
          <input
            className="input"
            value={memoryQuery}
            onChange={(e) => setMemoryQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void loadDailyMemory(); } }}
            placeholder={uiText("输入关键词，留空则显示最近记录", "Enter a keyword, or leave empty for recent entries", "キーワードを入力。空欄なら最近の記録を表示")}
            style={{ flex: "1 1 280px" }}
          />
          <button className="btn btn-primary btn-sm" onClick={() => void loadDailyMemory()} disabled={memoryLoading} style={{ gap: 6 }}>
            {memoryLoading ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Search size={14} />}
            {memoryQuery.trim() ? uiText("搜索", "Search", "検索") : uiText("最近记录", "Recent Entries", "最近の記録")}
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: 14 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {memoryLoading && memoryEntries.length === 0 ? (
              <div className="loading-center" style={{ minHeight: 180 }}><div className="spinner" /></div>
            ) : memoryEntries.length === 0 ? (
              <div className="card" style={{ padding: "18px 16px", border: "1px dashed var(--border-color)", background: "var(--bg-secondary)" }}>
                <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 6 }}>{uiText("没有匹配结果", "No matching entries", "一致する結果はありません")}</div>
                <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.6 }}>
                  {uiText("会扫描全局 `~/.openclaw` 与已发现项目目录中的 Daily Memory / Journal / Diary 文件。", "The search scans global `~/.openclaw` and discovered project Daily Memory / Journal / Diary files.", "グローバル `~/.openclaw` と検出済みプロジェクト内の Daily Memory / Journal / Diary を走査します。")}
                </div>
              </div>
            ) : (
              memoryEntries.map((entry) => {
                const active = entry.path === memorySelectedPath;
                return (
                  <button key={entry.path} type="button" className="card" onClick={() => void openMemoryEntry(entry)}
                    style={{ padding: "14px 16px", textAlign: "left", border: active ? "1px solid var(--accent-primary)" : "1px solid var(--border-color)", background: active ? "color-mix(in srgb, var(--accent-primary) 10%, var(--bg-secondary))" : "var(--bg-secondary)", display: "flex", flexDirection: "column", gap: 8 }}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start" }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: 12, fontWeight: 700, color: "var(--text-primary)", wordBreak: "break-word" }}>{entry.file_name}</div>
                        <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                          {entry.source === "global" ? uiText("全局", "Global", "グローバル") : uiText("项目", "Project", "プロジェクト")}
                          {entry.project_name ? ` · ${entry.project_name}` : ""}
                        </div>
                      </div>
                      {entry.modified_at && <span style={{ fontSize: 10, color: "var(--text-muted)", whiteSpace: "nowrap" }}>{entry.modified_at}</span>}
                    </div>
                    <div style={{ fontSize: 11, color: "var(--text-secondary)", lineHeight: 1.6 }}>{entry.preview}</div>
                    <div style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>{entry.path}</div>
                  </button>
                );
              })
            )}
          </div>

          <div className="card" style={{ padding: "14px 16px", minHeight: 280, background: "var(--bg-secondary)" }}>
            <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "center", marginBottom: 10, flexWrap: "wrap" }}>
              <div style={{ fontSize: 12, fontWeight: 700 }}>{uiText("全文预览", "Full Content", "全文プレビュー")}</div>
              {memorySelectedPath && <div style={{ fontSize: 10, color: "var(--text-muted)", wordBreak: "break-all" }}>{memorySelectedPath}</div>}
            </div>
            {memoryLoadingContent ? (
              <div className="loading-center" style={{ minHeight: 220 }}><div className="spinner" /></div>
            ) : memoryContent ? (
              <pre className="code-block" style={{ margin: 0, whiteSpace: "pre-wrap", maxHeight: 420, overflow: "auto", fontSize: 11 }}>{memoryContent}</pre>
            ) : (
              <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.7 }}>
                {uiText("选择左侧结果以查看 Daily Memory 全文。", "Select an entry on the left to inspect the full Daily Memory content.", "左側の結果を選択すると Daily Memory の全文を表示します。")}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function FieldCard({ label, input }: { label: string; input: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <label className="field-label">{label}</label>
      {input}
    </div>
  );
}
