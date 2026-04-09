import { lazy, memo, useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  RefreshCw, Save, X, ArrowRightLeft,
  Search, Terminal, Code, Monitor, Sparkles, Globe,
  Plus, Eye, EyeOff, Cat,
} from "lucide-react";
import { getLocale } from "../lib/i18n";
import {
  applyPresetToFields,
  buildStructuredConfig,
  createDefaultStructuredFields,
  getPresetCategories,
  parseStructuredConfig,
  supportsStructuredConfig,
  type ApiFormat,
  type ClaudeAuthField,
  type CodexReasoningEffort,
  type CodexWireApi,
  type OpenClawApiProtocol,
  type OpenCodeNpmPackage,
  type OpenCodeThinkingLevel,
  type PresetProviderType,
  type StructuredDraftFields,
} from "../lib/configProfiles";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import CopilotAuthSection from "../components/CopilotAuthSection";
import { useApplyConfigProfileMutation } from "../hooks/mutations";
import ProfileCard from "../components/ProfileCard";
import ProfileEditor from "../components/ProfileEditor";
import ProfileFragmentCard from "../components/ProfileFragmentCard";
import ProfilePresetButton from "../components/ProfilePresetButton";
import ProfileTargetToolToggle from "../components/ProfileTargetToolToggle";
import ProfileToolFilterTab from "../components/ProfileToolFilterTab";
const CodeEditor = lazy(() => import("../components/CodeEditor"));

interface ConfigProfile {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
  sort_order: number;
  source_type?: string | null;
  source_key?: string | null;
  created_at: string | null;
  updated_at: string | null;
}

interface ProviderConfigFragment {
  id: string;
  name: string;
  targetTools: string[];
  fields: Partial<StructuredDraftFields>;
  createdAt: string;
  updatedAt: string;
}

interface ProviderPingResult {
  profile_id: string;
  tool_id: string;
  provider_name: string;
  base_url: string | null;
  status: string;
  latency_ms: number | null;
  http_status: number | null;
  checked_at: string;
  message: string;
}

interface ProviderStreamCheckResult {
  profile_id: string;
  tool_id: string;
  provider_name: string;
  base_url: string | null;
  status: string;
  latency_ms: number | null;
  http_status: number | null;
  checked_at: string;
  message: string;
}

interface DetectedTool {
  id: string;
  name: string;
  installed: boolean;
}

const TOOL_ICONS: Record<string, typeof Monitor> = {
  claude: Terminal,
  codex: Code,
  gemini: Sparkles,
  opencode: Globe,
  openclaw: Cat,
};

const OPENCLAW_PROTOCOL_OPTIONS: OpenClawApiProtocol[] = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "bedrock-converse-stream",
];

const OPENCODE_NPM_OPTIONS: OpenCodeNpmPackage[] = [
  "@ai-sdk/openai",
  "@ai-sdk/openai-compatible",
  "@ai-sdk/anthropic",
  "@ai-sdk/amazon-bedrock",
  "@ai-sdk/google",
];

const CODEX_REASONING_OPTIONS: CodexReasoningEffort[] = ["low", "medium", "high", "xhigh"];
const CODEX_WIRE_API_OPTIONS: CodexWireApi[] = ["responses", "chat"];
const THINKING_LEVEL_OPTIONS: OpenCodeThinkingLevel[] = ["minimal", "low", "medium", "high"];
const SECTION_TITLE_STYLE = {
  fontSize: 13,
  fontWeight: 600,
  color: "var(--text-muted)",
  marginBottom: 12,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
} as const;
const FIELD_STACK_STYLE = {
  display: "flex",
  flexDirection: "column",
  gap: 6,
} as const;
const SMALL_INPUT_STYLE = { fontSize: 13 } as const;
const TWO_COLUMN_GRID_STYLE = { display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16 } as const;

function formatTime(value: string | null) {
  if (!value) return "";
  return value.replace("T", " ").slice(0, 19);
}

function prettyJson(content: string): string {
  try {
    return JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    return content;
  }
}

function getConfigLanguage(toolId: string, content: string): "json" | "toml" {
  if (toolId === "codex") {
    try {
      JSON.parse(content);
      return "json";
    } catch {
      return "toml";
    }
  }
  return "json";
}

function extractConfigSummary(toolId: string, content: string): { baseUrl?: string; model?: string } {
  try {
    const parsed = JSON.parse(content) as Record<string, any>;
    if (toolId === "claude") {
      const env = (parsed.env || {}) as Record<string, string>;
      return {
        baseUrl: env.ANTHROPIC_BASE_URL,
        model: env.ANTHROPIC_MODEL || env.ANTHROPIC_DEFAULT_SONNET_MODEL,
      };
    }
    if (toolId === "gemini") {
      const env = (parsed.env || {}) as Record<string, string>;
      return {
        baseUrl: env.GOOGLE_GEMINI_BASE_URL,
        model: env.GEMINI_MODEL,
      };
    }
    if (toolId === "codex") {
      const config = typeof parsed.config === "string" ? parsed.config : "";
      const modelMatch = config.match(/^model\s*=\s*"([^"]*)"/m);
      const urlMatch = config.match(/^base_url\s*=\s*"([^"]*)"/m);
      return {
        baseUrl: urlMatch?.[1],
        model: modelMatch?.[1],
      };
    }
    if (toolId === "openclaw") {
      const models = Array.isArray(parsed.models) ? parsed.models : [];
      const firstModel = models[0] as { id?: string } | undefined;
      return {
        baseUrl: parsed.baseUrl as string | undefined,
        model: firstModel?.id,
      };
    }
    if (toolId === "opencode") {
      const options = (parsed.options || {}) as Record<string, string>;
      const modelsObj = (parsed.models || {}) as Record<string, unknown>;
      const firstModelId = Object.keys(modelsObj)[0];
      return {
        baseUrl: options.baseURL,
        model: firstModelId,
      };
    }
  } catch { /* ignore */ }
  return {};
}

const CodexRawConfigEditor = memo(function CodexRawConfigEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  let authJson = "";
  let configToml = "";
  try {
    const parsed = JSON.parse(value) as Record<string, any>;
    authJson = JSON.stringify(parsed.auth || {}, null, 2);
    configToml = typeof parsed.config === "string" ? parsed.config : "";
  } catch {
    return <CodeEditor value={value} onChange={onChange} language="json" minHeight={240} />;
  }

  const rebuild = useCallback((nextAuth: string, nextToml: string) => {
    try {
      const auth = JSON.parse(nextAuth);
      onChange(JSON.stringify({ ...JSON.parse(value), auth, config: nextToml }, null, 2));
    } catch { /* ignore invalid JSON */ }
  }, [onChange, value]);

  const handleAuthChange = useCallback((nextAuth: string) => {
    rebuild(nextAuth, configToml);
  }, [configToml, rebuild]);

  const handleTomlChange = useCallback((nextToml: string) => {
    rebuild(authJson, nextToml);
  }, [authJson, rebuild]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <label className="field-label" style={{ marginBottom: 6 }}>auth.json</label>
        <CodeEditor
          value={authJson}
          onChange={handleAuthChange}
          language="json"
          minHeight={80}
        />
      </div>
      <div>
        <label className="field-label" style={{ marginBottom: 6 }}>config.toml</label>
        <CodeEditor
          value={configToml}
          onChange={handleTomlChange}
          language="toml"
          minHeight={200}
        />
      </div>
    </div>
  );
});

const SectionTitle = memo(function SectionTitle({ children }: { children: ReactNode }) {
  return (
    <h3 style={SECTION_TITLE_STYLE}>
      {children}
    </h3>
  );
});

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div style={FIELD_STACK_STYLE}>
      <label className="field-label">{label}</label>
      {children}
    </div>
  );
}

function TextInput(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input className="input" style={{ ...SMALL_INPUT_STYLE, ...(props.style || {}) }} {...props} />;
}


const SelectField = memo(function SelectField({
  value,
  onChange,
  options,
}: {
  value: string;
  onChange: (value: string) => void;
  options: string[];
}) {
  return (
    <select className="input" value={value} onChange={(e) => onChange(e.target.value)} style={SMALL_INPUT_STYLE}>
      {options.map((option) => (
        <option key={option} value={option}>{option}</option>
      ))}
    </select>
  );
});

interface ProfileConnectionSectionProps {
  locale: string;
  localeText: (zhText: string, enText: string, jaText?: string) => string;
  draftTool: string;
  draftProviderType: PresetProviderType | "";
  draftRequiresOAuth: boolean;
  draftOauthAccountId: string;
  showApiKey: boolean;
  draftApiKey: string;
  draftBaseUrl: string;
  draftCostMultiplier: string;
  draftEndpointCandidates: string;
  draftAuthField: ClaudeAuthField;
  draftApiFormat: ApiFormat;
  draftCodexReasoningEffort: CodexReasoningEffort;
  draftCodexWireApi: CodexWireApi;
  draftApiProtocol: OpenClawApiProtocol;
  draftModelCatalogAlias: string;
  draftNpm: OpenCodeNpmPackage;
  draftOpenCodeThinkingLevel: OpenCodeThinkingLevel | "";
  onDraftChange: (toolId: string, next: Partial<StructuredDraftFields>) => void;
  onAccountSelect: (accountId: string | null) => void;
  onToggleApiKeyVisibility: () => void;
}

interface ProfileBasicInfoSectionProps {
  locale: string;
  localeText: (zhText: string, enText: string, jaText?: string) => string;
  tools: DetectedTool[];
  draftTool: string;
  draftName: string;
  isStructured: boolean;
  syncTargetsLocked: boolean;
  draftTargetTools: string[];
  structuredInstalledTools: DetectedTool[];
  onToolChange: (event: ChangeEvent<HTMLSelectElement>) => void;
  onNameChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onToggleDraftTargetTool: (toolId: string) => void;
}

const ProfileBasicInfoSection = memo(function ProfileBasicInfoSection({
  locale,
  localeText,
  tools,
  draftTool,
  draftName,
  isStructured,
  syncTargetsLocked,
  draftTargetTools,
  structuredInstalledTools,
  onToolChange,
  onNameChange,
  onToggleDraftTargetTool,
}: ProfileBasicInfoSectionProps) {
  return (
    <div>
      <SectionTitle>{locale === "zh" ? "基本信息" : "Basic Info"}</SectionTitle>
      <div style={TWO_COLUMN_GRID_STYLE}>
        <Field label={locale === "zh" ? "工具" : "Tool"}>
          <select
            className="input"
            value={draftTool}
            disabled={syncTargetsLocked}
            onChange={onToolChange}
            style={SMALL_INPUT_STYLE}
          >
            {tools.map((tool) => (
              <option key={tool.id} value={tool.id}>{tool.name}</option>
            ))}
          </select>
        </Field>
        <Field label={locale === "zh" ? "配置名称" : "Name"}>
          <TextInput
            placeholder={locale === "zh" ? "例如：官方 API、中转服务" : "e.g. Official API, Proxy Service"}
            value={draftName}
            onChange={onNameChange}
            autoFocus
          />
        </Field>
      </div>
      {isStructured && (
        <div style={{ marginTop: 16 }}>
          <Field label={localeText("同步到 App", "Sync to Apps", "App へ同期")}>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {structuredInstalledTools.map((tool) => {
                const selected = draftTargetTools.includes(tool.id);
                return (
                  <ProfileTargetToolToggle
                    key={tool.id}
                    toolId={tool.id}
                    toolName={tool.name}
                    selected={selected}
                    disabled={selected && draftTargetTools.length === 1}
                    onToggle={onToggleDraftTargetTool}
                  />
                );
              })}
            </div>
            <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 8 }}>
              {draftTargetTools.length > 1
                ? localeText(
                  "保存后会把同名 Provider 作为共享组同步到所选 App，编辑任一成员时会联动更新整组。",
                  "Saving will sync this provider as a shared group across the selected apps. Editing any member updates the whole group.",
                  "保存すると、選択した App に共有グループとして同期されます。任意のメンバーを編集するとグループ全体が更新されます。",
                )
                : localeText(
                  "当前仅保存到单个 App。选择多个 App 后会启用共享 Provider 同步。",
                  "This will save to a single app. Select multiple apps to enable shared provider syncing.",
                  "現在は単一 App にのみ保存されます。複数 App を選ぶと共有 Provider 同期が有効になります。",
                )}
            </div>
          </Field>
        </div>
      )}
    </div>
  );
});

interface ProfilePresetSectionProps {
  locale: string;
  localeText: (zhText: string, enText: string, jaText?: string) => string;
  draftTool: string;
  draftPresetId: string;
  presetCategories: ReturnType<typeof getPresetCategories>;
  draftFragmentName: string;
  savingFragment: boolean;
  providerFragments: ProviderConfigFragment[];
  toolNameMap: Record<string, string>;
  deletingFragmentId: string | null;
  onPresetApply: (presetId: string) => void;
  onFragmentNameChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onSaveFragment: () => void;
  onApplyFragment: (fragmentId: string) => void;
  onDeleteFragment: (fragmentId: string) => void;
}

const ProfilePresetSection = memo(function ProfilePresetSection({
  locale,
  localeText,
  draftTool,
  draftPresetId,
  presetCategories,
  draftFragmentName,
  savingFragment,
  providerFragments,
  toolNameMap,
  deletingFragmentId,
  onPresetApply,
  onFragmentNameChange,
  onSaveFragment,
  onApplyFragment,
  onDeleteFragment,
}: ProfilePresetSectionProps) {
  return (
    <>
      <div>
        <SectionTitle>{locale === "zh" ? "预设模板" : "Preset"}</SectionTitle>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {presetCategories.flatMap((group) =>
            group.presets.map((preset) => (
              <ProfilePresetButton
                key={preset.id}
                presetId={preset.id}
                name={preset.name}
                badge={preset.badge}
                active={draftPresetId === preset.id}
                onApply={onPresetApply}
              />
            )),
          )}
        </div>
      </div>

      <div>
        <SectionTitle>{localeText("公共配置片段", "Shared Fragments", "共有フラグメント")}</SectionTitle>
        <div className="card" style={{ padding: 14, display: "flex", flexDirection: "column", gap: 14 }}>
          <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", gap: 12, alignItems: "end" }}>
            <Field label={localeText("片段名称", "Fragment Name", "フラグメント名")}>
              <TextInput
                value={draftFragmentName}
                onChange={onFragmentNameChange}
                placeholder={localeText("例如：OpenAI 兼容基础参数", "e.g. OpenAI-compatible defaults", "例: OpenAI 互換の基本設定")}
              />
            </Field>
            <button
              className="btn btn-secondary btn-sm"
              type="button"
              onClick={onSaveFragment}
              disabled={!draftFragmentName.trim() || savingFragment}
              style={{ gap: 6 }}
            >
              {savingFragment ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Save size={14} />}
              {localeText("保存当前表单", "Save Current Form", "現在のフォームを保存")}
            </button>
          </div>
          <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
            {localeText(
              "会保存当前结构化字段，后续可在不同 Provider 草稿间复用；应用时保留当前编辑中的 App 同步目标。",
              "This saves the current structured fields for reuse across provider drafts. Applying a fragment keeps the current app sync targets.",
              "現在の構造化フィールドを保存し、別の Provider 下書きにも再利用できます。適用しても現在の App 同期先は維持されます。",
            )}
          </div>
          {providerFragments.length === 0 ? (
            <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
              {localeText(
                "还没有公共配置片段。保存一份当前表单后，就可以在这里一键复用。",
                "No shared fragments yet. Save the current form to reuse it here.",
                "共有フラグメントはまだありません。現在のフォームを保存すると、ここから再利用できます。",
              )}
            </div>
          ) : (
            providerFragments.map((fragment) => (
              <ProfileFragmentCard
                key={fragment.id}
                fragmentId={fragment.id}
                name={fragment.name}
                targetTools={fragment.targetTools}
                toolNameMap={toolNameMap}
                currentToolCompatible={fragment.targetTools.includes(draftTool)}
                compatibilityReadyLabel={localeText("含当前工具字段", "Includes current tool fields", "現在のツール向け字段あり")}
                compatibilityCommonOnlyLabel={localeText("仅应用通用字段", "Common fields only", "共通フィールドのみ")}
                updatedLabel={localeText("最近更新", "Updated", "更新日時")}
                updatedAt={formatTime(fragment.updatedAt)}
                applyLabel={localeText("应用", "Apply", "適用")}
                deleteTitle={localeText("删除片段", "Delete fragment", "フラグメントを削除")}
                deleting={deletingFragmentId === fragment.id}
                onApply={onApplyFragment}
                onDelete={onDeleteFragment}
              />
            ))
          )}
        </div>
      </div>
    </>
  );
});

const ProfileConnectionSection = memo(function ProfileConnectionSection({
  locale,
  localeText,
  draftTool,
  draftProviderType,
  draftRequiresOAuth,
  draftOauthAccountId,
  showApiKey,
  draftApiKey,
  draftBaseUrl,
  draftCostMultiplier,
  draftEndpointCandidates,
  draftAuthField,
  draftApiFormat,
  draftCodexReasoningEffort,
  draftCodexWireApi,
  draftApiProtocol,
  draftModelCatalogAlias,
  draftNpm,
  draftOpenCodeThinkingLevel,
  onDraftChange,
  onAccountSelect,
  onToggleApiKeyVisibility,
}: ProfileConnectionSectionProps) {
  return (
    <div>
      <SectionTitle>{locale === "zh" ? "连接配置" : "Connection"}</SectionTitle>
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        {draftProviderType === "github_copilot" && (
          <CopilotAuthSection
            selectedAccountId={draftOauthAccountId || null}
            onAccountSelect={onAccountSelect}
            showDescription={false}
          />
        )}
        {draftRequiresOAuth && (
          <div className="card" style={{ padding: 12, fontSize: 12, color: "var(--text-muted)" }}>
            {draftProviderType === "github_copilot"
              ? localeText(
                "当前预设使用 GitHub Copilot OAuth。无需填写 API Key；请先登录 GitHub 账号，并在需要时绑定指定账号。实际使用时建议在 Settings 中启用 Claude 的本地代理。",
                "This preset uses GitHub Copilot OAuth. No API key is required; sign in with GitHub and optionally bind a specific account. Enable the Claude local proxy in Settings when using the provider.",
                "このプリセットは GitHub Copilot OAuth を使用します。API Key は不要です。GitHub にログインし、必要なら特定アカウントを紐付けてください。利用時は Settings で Claude のローカルプロキシを有効にすることを推奨します。",
              )
              : localeText(
                "当前预设使用 OAuth 模式，无需填写 API Key。",
                "This preset uses OAuth mode and does not require an API key.",
                "このプリセットは OAuth モードのため API Key は不要です。",
              )}
          </div>
        )}
        {!draftRequiresOAuth && (
          <Field label="API Key">
            <div style={{ position: "relative" }}>
              <TextInput
                type={showApiKey ? "text" : "password"}
                value={draftApiKey}
                onChange={(event) => onDraftChange(draftTool, { apiKey: event.target.value })}
                placeholder={locale === "zh" ? "填写 API Key" : "Enter API Key"}
                style={{ paddingRight: 40 }}
              />
              <button
                className="btn btn-ghost btn-icon-sm"
                style={{ position: "absolute", right: 4, top: "50%", transform: "translateY(-50%)" }}
                onClick={onToggleApiKeyVisibility}
                type="button"
              >
                {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
            </div>
          </Field>
        )}

        <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) minmax(180px, 240px)", gap: 16 }}>
          <Field label={locale === "zh" ? "接口地址" : "Base URL"}>
            <TextInput
              value={draftBaseUrl}
              onChange={(event) => onDraftChange(draftTool, { baseUrl: event.target.value })}
              placeholder="https://api.example.com"
            />
          </Field>
          <Field label={localeText("成本倍率", "Cost Multiplier", "コスト倍率")}>
            <TextInput
              value={draftCostMultiplier}
              onChange={(event) => onDraftChange(draftTool, { costMultiplier: event.target.value })}
              placeholder="1.0"
            />
          </Field>
        </div>

        <Field label={localeText("候选端点", "Endpoint Candidates", "候補エンドポイント")}>
          <textarea
            className="input"
            value={draftEndpointCandidates}
            onChange={(event) => onDraftChange(draftTool, { endpointCandidates: event.target.value })}
            placeholder={localeText(
              "每行一个备用地址，例如：\nhttps://api.example.com\nhttps://backup.example.com",
              "One backup URL per line, for example:\nhttps://api.example.com\nhttps://backup.example.com",
              "1 行につき 1 つの予備 URL を入力します。例:\nhttps://api.example.com\nhttps://backup.example.com",
            )}
            style={{ minHeight: 88, resize: "vertical", fontSize: 13 }}
          />
        </Field>

        {draftTool === "claude" && (
          <div style={TWO_COLUMN_GRID_STYLE}>
            <Field label={locale === "zh" ? "认证字段 (默认 AUTH_TOKEN)" : "Auth Field (default AUTH_TOKEN)"}>
              <SelectField
                value={draftAuthField}
                onChange={(value) => onDraftChange(draftTool, { authField: value as ClaudeAuthField })}
                options={["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_API_KEY"]}
              />
            </Field>
            <Field label={locale === "zh" ? "API 格式 (默认 anthropic)" : "API Format (default anthropic)"}>
              <SelectField
                value={draftApiFormat}
                onChange={(value) => onDraftChange(draftTool, { apiFormat: value as ApiFormat })}
                options={["anthropic", "openai_chat", "openai_responses"]}
              />
            </Field>
          </div>
        )}

        {draftTool === "codex" && (
          <div style={TWO_COLUMN_GRID_STYLE}>
            <Field label={locale === "zh" ? "推理强度" : "Reasoning Effort"}>
              <SelectField
                value={draftCodexReasoningEffort}
                onChange={(value) => onDraftChange(draftTool, { codexReasoningEffort: value as CodexReasoningEffort })}
                options={CODEX_REASONING_OPTIONS}
              />
            </Field>
            <Field label={locale === "zh" ? "Wire API" : "Wire API"}>
              <SelectField
                value={draftCodexWireApi}
                onChange={(value) => onDraftChange(draftTool, { codexWireApi: value as CodexWireApi })}
                options={CODEX_WIRE_API_OPTIONS}
              />
            </Field>
          </div>
        )}

        {draftTool === "openclaw" && (
          <div style={TWO_COLUMN_GRID_STYLE}>
            <Field label={locale === "zh" ? "API 协议" : "API Protocol"}>
              <SelectField
                value={draftApiProtocol}
                onChange={(value) => onDraftChange(draftTool, { apiProtocol: value as OpenClawApiProtocol })}
                options={OPENCLAW_PROTOCOL_OPTIONS}
              />
            </Field>
            <Field label={locale === "zh" ? "模型别名" : "Model Alias"}>
              <TextInput
                value={draftModelCatalogAlias}
                onChange={(event) => onDraftChange(draftTool, { modelCatalogAlias: event.target.value })}
                placeholder="DeepSeek"
              />
            </Field>
          </div>
        )}

        {draftTool === "opencode" && (
          <div style={TWO_COLUMN_GRID_STYLE}>
            <Field label={locale === "zh" ? "NPM 包" : "NPM Package"}>
              <SelectField
                value={draftNpm}
                onChange={(value) => onDraftChange(draftTool, { npm: value as OpenCodeNpmPackage })}
                options={OPENCODE_NPM_OPTIONS}
              />
            </Field>
            <Field label={locale === "zh" ? "Thinking Level" : "Thinking Level"}>
              <select
                className="input"
                value={draftOpenCodeThinkingLevel}
                onChange={(event) => onDraftChange(draftTool, {
                  openCodeThinkingLevel: event.target.value as OpenCodeThinkingLevel | "",
                })}
                style={SMALL_INPUT_STYLE}
              >
                <option value="">{locale === "zh" ? "无" : "None"}</option>
                {THINKING_LEVEL_OPTIONS.map((option) => <option key={option} value={option}>{option}</option>)}
              </select>
            </Field>
          </div>
        )}
      </div>
    </div>
  );
});

interface ProfileModelsSectionProps {
  locale: string;
  draftTool: string;
  draftModel: string;
  draftReasoningModel: string;
  draftHaikuModel: string;
  draftSonnetModel: string;
  draftOpusModel: string;
  draftModelName: string;
  draftOpenCodeContextLimit: string;
  draftOpenCodeOutputLimit: string;
  draftOpenCodeInputModalities: string;
  draftOpenCodeOutputModalities: string;
  onDraftChange: (toolId: string, next: Partial<StructuredDraftFields>) => void;
}

const ProfileModelsSection = memo(function ProfileModelsSection({
  locale,
  draftTool,
  draftModel,
  draftReasoningModel,
  draftHaikuModel,
  draftSonnetModel,
  draftOpusModel,
  draftModelName,
  draftOpenCodeContextLimit,
  draftOpenCodeOutputLimit,
  draftOpenCodeInputModalities,
  draftOpenCodeOutputModalities,
  onDraftChange,
}: ProfileModelsSectionProps) {
  return (
    <div>
      <SectionTitle>{locale === "zh" ? "模型配置" : "Models"}</SectionTitle>
      {draftTool === "claude" ? (
        <div style={TWO_COLUMN_GRID_STYLE}>
          <Field label={locale === "zh" ? "主模型" : "Main Model"}>
            <TextInput value={draftModel} onChange={(event) => onDraftChange(draftTool, { model: event.target.value })} placeholder="claude-sonnet-4-5" />
          </Field>
          <Field label={locale === "zh" ? "推理模型" : "Reasoning Model"}>
            <TextInput value={draftReasoningModel} onChange={(event) => onDraftChange(draftTool, { reasoningModel: event.target.value })} placeholder="claude-sonnet-4-5" />
          </Field>
          <Field label={locale === "zh" ? "Haiku 默认模型" : "Default Haiku"}>
            <TextInput value={draftHaikuModel} onChange={(event) => onDraftChange(draftTool, { haikuModel: event.target.value })} placeholder="claude-haiku-3-5" />
          </Field>
          <Field label={locale === "zh" ? "Sonnet 默认模型" : "Default Sonnet"}>
            <TextInput value={draftSonnetModel} onChange={(event) => onDraftChange(draftTool, { sonnetModel: event.target.value })} placeholder="claude-sonnet-4-5" />
          </Field>
          <Field label={locale === "zh" ? "Opus 默认模型" : "Default Opus"}>
            <TextInput value={draftOpusModel} onChange={(event) => onDraftChange(draftTool, { opusModel: event.target.value })} placeholder="claude-opus-4-5" />
          </Field>
        </div>
      ) : (
        <div style={TWO_COLUMN_GRID_STYLE}>
          <Field label={locale === "zh" ? "模型 ID" : "Model ID"}>
            <TextInput
              value={draftModel}
              onChange={(event) => onDraftChange(draftTool, { model: event.target.value })}
              placeholder={locale === "zh" ? "例如 deepseek-chat" : "e.g. deepseek-chat"}
            />
          </Field>
          <Field label={locale === "zh" ? "模型显示名" : "Display Name"}>
            <TextInput
              value={draftModelName}
              onChange={(event) => onDraftChange(draftTool, { modelName: event.target.value })}
              placeholder={locale === "zh" ? "可选，默认同 ID" : "Optional, defaults to ID"}
            />
          </Field>
          {draftTool === "opencode" && (
            <>
              <Field label={locale === "zh" ? "Context Limit" : "Context Limit"}>
                <TextInput value={draftOpenCodeContextLimit} onChange={(event) => onDraftChange(draftTool, { openCodeContextLimit: event.target.value })} placeholder="400000" />
              </Field>
              <Field label={locale === "zh" ? "Output Limit" : "Output Limit"}>
                <TextInput value={draftOpenCodeOutputLimit} onChange={(event) => onDraftChange(draftTool, { openCodeOutputLimit: event.target.value })} placeholder="128000" />
              </Field>
              <Field label={locale === "zh" ? "输入模态" : "Input Modalities"}>
                <TextInput value={draftOpenCodeInputModalities} onChange={(event) => onDraftChange(draftTool, { openCodeInputModalities: event.target.value })} placeholder="text,image,pdf" />
              </Field>
              <Field label={locale === "zh" ? "输出模态" : "Output Modalities"}>
                <TextInput value={draftOpenCodeOutputModalities} onChange={(event) => onDraftChange(draftTool, { openCodeOutputModalities: event.target.value })} placeholder="text" />
              </Field>
            </>
          )}
        </div>
      )}
    </div>
  );
});

interface ProfileRawConfigSectionProps {
  locale: string;
  draftTool: string;
  draftContent: string;
  draftHideAttribution: boolean;
  draftEffortHigh: boolean;
  draftEnableTeammates: boolean;
  onDraftChange: (toolId: string, next: Partial<StructuredDraftFields>) => void;
  onContentChange: (value: string) => void;
}

interface ProfilePlainConfigSectionProps {
  locale: string;
  draftTool: string;
  draftContent: string;
  draftLoading: boolean;
  onContentChange: (value: string) => void;
}

const ProfileRawConfigSection = memo(function ProfileRawConfigSection({
  locale,
  draftTool,
  draftContent,
  draftHideAttribution,
  draftEffortHigh,
  draftEnableTeammates,
  onDraftChange,
  onContentChange,
}: ProfileRawConfigSectionProps) {
  return (
    <div>
      <SectionTitle>{locale === "zh" ? "原始配置" : "Raw Configuration"}</SectionTitle>
      {draftTool === "claude" && (
        <div style={{ display: "flex", gap: 20, flexWrap: "wrap", marginBottom: 12 }}>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <input type="checkbox" checked={draftHideAttribution} onChange={(event) => onDraftChange(draftTool, { hideAttribution: event.target.checked })} />
            {locale === "zh" ? "隐藏 AI 署名" : "Hide AI Attribution"}
          </label>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <input type="checkbox" checked={draftEffortHigh} onChange={(event) => onDraftChange(draftTool, { effortHigh: event.target.checked })} />
            {locale === "zh" ? "高强度思考" : "High Effort Thinking"}
          </label>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <input type="checkbox" checked={draftEnableTeammates} onChange={(event) => onDraftChange(draftTool, { enableTeammates: event.target.checked })} />
            {locale === "zh" ? "Teammates 模式" : "Teammates Mode"}
          </label>
        </div>
      )}
      <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
        {locale === "zh" ? "上方表单字段会自动同步到此处，你也可以直接编辑原始配置。" : "Form fields above are synced here. You can also edit the raw config directly."}
      </div>
      {draftTool === "codex" ? (
        <CodexRawConfigEditor value={draftContent} onChange={onContentChange} />
      ) : (
        <CodeEditor value={draftContent} onChange={onContentChange} language={getConfigLanguage(draftTool, draftContent)} minHeight={240} />
      )}
    </div>
  );
});

const ProfilePlainConfigSection = memo(function ProfilePlainConfigSection({
  locale,
  draftTool,
  draftContent,
  draftLoading,
  onContentChange,
}: ProfilePlainConfigSectionProps) {
  return (
    <div>
      <SectionTitle>{locale === "zh" ? "配置内容" : "Configuration"}</SectionTitle>
      <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
        {locale === "zh" ? "直接编辑完整配置内容。" : "Edit the full configuration directly."}
      </div>
      {draftLoading ? (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: 200 }}>
          <div className="spinner" />
        </div>
      ) : (
        <CodeEditor
          value={draftContent}
          onChange={onContentChange}
          language={getConfigLanguage(draftTool, draftContent)}
          minHeight={300}
        />
      )}
    </div>
  );
});

function mergeSharedDraftFields(
  current: StructuredDraftFields,
  toolId: string,
  parsed: StructuredDraftFields,
  includeCommon: boolean,
  includeToolSpecific = true,
): StructuredDraftFields {
  const next = { ...current };

  if (includeCommon) {
    next.baseUrl = parsed.baseUrl || next.baseUrl;
    next.apiKey = parsed.apiKey || next.apiKey;
    next.model = parsed.model || next.model;
    next.websiteUrl = parsed.websiteUrl || next.websiteUrl;
    next.apiKeyUrl = parsed.apiKeyUrl || next.apiKeyUrl;
    next.category = parsed.category || next.category;
    next.endpointCandidates = parsed.endpointCandidates || next.endpointCandidates;
    next.costMultiplier = parsed.costMultiplier || next.costMultiplier;
    next.requiresOAuth = parsed.requiresOAuth || next.requiresOAuth;
    next.providerType = parsed.providerType || next.providerType;
    next.oauthAccountId = parsed.oauthAccountId || next.oauthAccountId;
  }

  if (!includeToolSpecific) {
    return next;
  }

  if (toolId === "claude") {
    next.reasoningModel = parsed.reasoningModel;
    next.haikuModel = parsed.haikuModel;
    next.sonnetModel = parsed.sonnetModel;
    next.opusModel = parsed.opusModel;
    next.authField = parsed.authField;
    next.apiFormat = parsed.apiFormat;
    next.hideAttribution = parsed.hideAttribution;
    next.effortHigh = parsed.effortHigh;
    next.enableTeammates = parsed.enableTeammates;
  } else if (toolId === "codex") {
    next.codexWireApi = parsed.codexWireApi;
    next.codexReasoningEffort = parsed.codexReasoningEffort;
  } else if (toolId === "openclaw") {
    next.apiProtocol = parsed.apiProtocol;
    next.modelName = parsed.modelName;
    next.openClawContextWindow = parsed.openClawContextWindow;
    next.openClawCostInput = parsed.openClawCostInput;
    next.openClawCostOutput = parsed.openClawCostOutput;
    next.suggestedPrimaryModel = parsed.suggestedPrimaryModel;
    next.suggestedFallbackModels = parsed.suggestedFallbackModels;
    next.modelCatalogAlias = parsed.modelCatalogAlias;
  } else if (toolId === "opencode") {
    next.npm = parsed.npm;
    next.modelName = parsed.modelName || next.modelName;
    next.openCodeContextLimit = parsed.openCodeContextLimit;
    next.openCodeOutputLimit = parsed.openCodeOutputLimit;
    next.openCodeInputModalities = parsed.openCodeInputModalities;
    next.openCodeOutputModalities = parsed.openCodeOutputModalities;
    next.openCodeVariantName = parsed.openCodeVariantName;
    next.openCodeIncludeThoughts = parsed.openCodeIncludeThoughts;
    next.openCodeThinkingBudget = parsed.openCodeThinkingBudget;
    next.openCodeThinkingLevel = parsed.openCodeThinkingLevel;
    next.openCodeReasoningEffort = parsed.openCodeReasoningEffort;
    next.openCodeEffort = parsed.openCodeEffort;
  }

  return next;
}

function mergeDraftFields(
  current: StructuredDraftFields,
  next: Partial<StructuredDraftFields> = {},
): StructuredDraftFields {
  const merged = { ...current } as StructuredDraftFields;
  for (const [key, value] of Object.entries(next) as [keyof StructuredDraftFields, StructuredDraftFields[keyof StructuredDraftFields] | undefined][]) {
    if (value !== undefined) {
      ((merged as unknown) as Record<string, unknown>)[key] = value;
    }
  }
  return merged;
}

type DraftFieldsStateUpdate =
  | StructuredDraftFields
  | ((current: StructuredDraftFields) => StructuredDraftFields);

export default function Profiles() {
  const [profiles, setProfiles] = useState<ConfigProfile[]>([]);
  const [tools, setTools] = useState<DetectedTool[]>([]);
  const [activeIds, setActiveIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [newTool, setNewTool] = useState("claude");
  const [saving, setSaving] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingProfile, setEditingProfile] = useState<ConfigProfile | null>(null);
  const [draftName, setDraftName] = useState("");
  const [draftTool, setDraftTool] = useState("claude");
  const [draftTargetTools, setDraftTargetTools] = useState<string[]>(["claude"]);
  const [draftContent, setDraftContent] = useState("");
  const [draftLoading, setDraftLoading] = useState(false);
  const [draftFields, setDraftFieldsState] = useState<StructuredDraftFields>(() => createDefaultStructuredFields("claude"));
  const [providerFragments, setProviderFragments] = useState<ProviderConfigFragment[]>([]);
  const [draftFragmentName, setDraftFragmentName] = useState("");
  const [savingFragment, setSavingFragment] = useState(false);
  const [deletingFragmentId, setDeletingFragmentId] = useState<string | null>(null);
  const [confirmFragmentDelete, setConfirmFragmentDelete] = useState<ProviderConfigFragment | null>(null);
  const [filterTool, setFilterTool] = useState("claude");
  const [search, setSearch] = useState("");
  const [showApiKey, setShowApiKey] = useState(false);
  const [draggingProfileId, setDraggingProfileId] = useState<string | null>(null);
  const [dragOverProfileId, setDragOverProfileId] = useState<string | null>(null);
  const [pingingId, setPingingId] = useState<string | null>(null);
  const [pingResults, setPingResults] = useState<Record<string, ProviderPingResult>>({});
  const [streamCheckingId, setStreamCheckingId] = useState<string | null>(null);
  const [streamCheckResults, setStreamCheckResults] = useState<Record<string, ProviderStreamCheckResult>>({});
  const [streamCheckConfirmProfile, setStreamCheckConfirmProfile] = useState<ConfigProfile | null>(null);

  const {
    presetId: draftPresetId,
    baseUrl: draftBaseUrl,
    apiKey: draftApiKey,
    model: draftModel,
    reasoningModel: draftReasoningModel,
    haikuModel: draftHaikuModel,
    sonnetModel: draftSonnetModel,
    opusModel: draftOpusModel,
    authField: draftAuthField,
    apiFormat: draftApiFormat,
    apiProtocol: draftApiProtocol,
    modelName: draftModelName,
    npm: draftNpm,
    endpointCandidates: draftEndpointCandidates,
    costMultiplier: draftCostMultiplier,
    requiresOAuth: draftRequiresOAuth,
    providerType: draftProviderType,
    oauthAccountId: draftOauthAccountId,
    hideAttribution: draftHideAttribution,
    effortHigh: draftEffortHigh,
    enableTeammates: draftEnableTeammates,
    codexWireApi: draftCodexWireApi,
    codexReasoningEffort: draftCodexReasoningEffort,
    modelCatalogAlias: draftModelCatalogAlias,
    openCodeContextLimit: draftOpenCodeContextLimit,
    openCodeOutputLimit: draftOpenCodeOutputLimit,
    openCodeInputModalities: draftOpenCodeInputModalities,
    openCodeOutputModalities: draftOpenCodeOutputModalities,
    openCodeThinkingLevel: draftOpenCodeThinkingLevel,
  } = draftFields;

  const [confirmAction, setConfirmAction] = useState<{ type: string; profile: ConfigProfile } | null>(null);
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const draftFieldsRef = useRef(draftFields);
  const shortcutFlagsRef = useRef({ canSave: false, isEditing: false });
  const shortcutHandlersRef = useRef<{
    save: () => Promise<void>;
    create: () => Promise<void>;
  }>({
    save: async () => {},
    create: async () => {},
  });
  const applyConfigProfileMutation = useApplyConfigProfileMutation();
  const localeText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);
  const installedTools = useMemo(() => tools.filter((tool) => tool.installed), [tools]);

  useEffect(() => {
    draftFieldsRef.current = draftFields;
  }, [draftFields]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      await invoke("sync_config_profiles");
      const [nextProfiles, nextTools, nextActiveIds, nextFragments] = await Promise.all([
        invoke<ConfigProfile[]>("get_config_profiles"),
        invoke<DetectedTool[]>("detect_tools"),
        invoke<string[]>("get_active_config_profile_ids"),
        invoke<ProviderConfigFragment[]>("get_provider_config_fragments").catch(() => [] as ProviderConfigFragment[]),
      ]);
      setProfiles(nextProfiles);
      setTools(nextTools);
      setActiveIds(nextActiveIds);
      setProviderFragments(nextFragments);
      await invoke("refresh_tray_provider_menu").catch(() => undefined);
      setNewTool((prev) => {
        const installed = nextTools.filter((tool) => tool.installed);
        if (installed.some((tool) => tool.id === prev)) return prev;
        return installed[0]?.id || nextTools[0]?.id || "claude";
      });
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `加载失败: ${e}` : `Load failed: ${e}`);
    } finally {
      setLoading(false);
    }
  }, [locale]);

  const updateDraftFieldsState = useCallback((next: DraftFieldsStateUpdate) => {
    setDraftFieldsState((current) => {
      const resolved = typeof next === "function"
        ? next(current)
        : next;
      draftFieldsRef.current = resolved;
      return resolved;
    });
  }, []);

  const setDraftFields = useCallback((fields: StructuredDraftFields) => {
    updateDraftFieldsState(fields);
  }, [updateDraftFieldsState]);

  const buildCurrentFields = useCallback((next: Partial<StructuredDraftFields> = {}): StructuredDraftFields => {
    return mergeDraftFields(draftFieldsRef.current, next);
  }, []);

  const updateStructuredDraft = useCallback((toolId: string, next: Partial<StructuredDraftFields>) => {
    const fields = buildCurrentFields(next);
    setDraftFields(fields);
    setDraftContent(buildStructuredConfig(toolId, fields));
  }, [buildCurrentFields]);

  function sortProviderFragments(fragments: ProviderConfigFragment[]) {
    return [...fragments].sort((a, b) =>
      b.updatedAt.localeCompare(a.updatedAt) || a.name.localeCompare(b.name),
    );
  }

  function normalizeFragmentFields(fragment: ProviderConfigFragment): StructuredDraftFields {
    return {
      ...createDefaultStructuredFields(draftTool),
      ...(fragment.fields || {}),
    };
  }

  const resetStructuredDraft = useCallback((toolId: string) => {
    const defaults = createDefaultStructuredFields(toolId);
    setDraftFields(defaults);
    setDraftContent(buildStructuredConfig(toolId, defaults));
    setDraftLoading(false);
  }, [setDraftFields]);

  const openCreateModal = useCallback(async (toolId?: string) => {
    const selectedTool = toolId || newTool;
    if (!installedTools.length) {
      showToast("error", locale === "zh" ? "没有可用工具配置" : "No available tool configuration");
      return;
    }
    setEditingProfile(null);
    setDraftName("");
    setDraftTool(selectedTool);
    setDraftTargetTools([selectedTool]);
    setDraftContent("");
    setShowCreateModal(true);
    setSaving(false);
    setNewTool(selectedTool);
    setShowApiKey(false);
    updateDraftFieldsState((current) => ({ ...current, apiFormat: "anthropic" }));
    if (supportsStructuredConfig(selectedTool)) {
      resetStructuredDraft(selectedTool);
      return;
    }
    setDraftContent("");
    setDraftLoading(true);
    try {
      const configContent = await invoke<string>("read_tool_config", { toolId: selectedTool });
      setDraftContent(prettyJson(configContent));
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `读取配置失败: ${e}` : `Failed to read configuration: ${e}`);
    } finally {
      setDraftLoading(false);
    }
  }, [installedTools.length, locale, newTool, resetStructuredDraft, updateDraftFieldsState]);

  const openEditModal = useCallback((profile: ConfigProfile) => {
    const sharedProfiles = profile.source_type === "shared" && profile.source_key
      ? profiles.filter((item) => item.source_type === "shared" && item.source_key === profile.source_key)
      : [profile];
    const otherProfiles = sharedProfiles.filter((item) => item.id !== profile.id);
    setEditingProfile(profile);
    setShowCreateModal(false);
    setDraftName(profile.name);
    setDraftTool(profile.tool_id);
    setDraftTargetTools(sharedProfiles.map((item) => item.tool_id));
    setDraftContent(prettyJson(profile.config_snapshot));
    setShowApiKey(false);
    if (supportsStructuredConfig(profile.tool_id)) {
      let merged = createDefaultStructuredFields(profile.tool_id);
      for (const item of otherProfiles) {
        if (!supportsStructuredConfig(item.tool_id)) continue;
        merged = mergeSharedDraftFields(
          merged,
          item.tool_id,
          parseStructuredConfig(item.tool_id, item.config_snapshot),
          false,
        );
      }
      merged = mergeSharedDraftFields(
        merged,
        profile.tool_id,
        parseStructuredConfig(profile.tool_id, profile.config_snapshot),
        true,
      );
      setDraftFields(merged);
      setDraftContent(buildStructuredConfig(profile.tool_id, merged));
    } else {
      resetStructuredDraft("claude");
    }
    setDraftLoading(false);
  }, [profiles, resetStructuredDraft, setDraftFields]);

  const closeModal = useCallback(() => {
    setShowCreateModal(false);
    setEditingProfile(null);
    setDraftName("");
    setDraftTargetTools(["claude"]);
    setDraftContent("");
    setDraftLoading(false);
    setDraftFields(createDefaultStructuredFields("claude"));
    setDraftFragmentName("");
    setSaving(false);
    setShowApiKey(false);
  }, [setDraftFields]);

  const doDeleteFragment = useCallback(async (fragment: ProviderConfigFragment) => {
    setDeletingFragmentId(fragment.id);
    try {
      await invoke("delete_provider_config_fragment", { id: fragment.id });
      setProviderFragments((current) => current.filter((item) => item.id !== fragment.id));
      showToast(
        "success",
        localeText("配置片段已删除", "Provider fragment deleted", "Provider フラグメントを削除しました"),
      );
    } catch (e) {
      console.error(e);
      showToast(
        "error",
        localeText(`删除片段失败: ${e}`, `Failed to delete fragment: ${e}`, `フラグメントの削除に失敗しました: ${e}`),
      );
    } finally {
      setDeletingFragmentId((current) => current === fragment.id ? null : current);
    }
  }, [localeText]);

  const handleSaveModal = useCallback(async () => {
    if (!draftName.trim() || saving) return;
    setSaving(true);
    try {
      if (supportsStructuredConfig(draftTool) && (draftTargetTools.length > 1 || editingProfile?.source_type === "shared")) {
        const targetTools = draftTargetTools.filter((toolId) => supportsStructuredConfig(toolId));
        const profilesPayload = targetTools.map((toolId) => ({
          toolId,
          configSnapshot: buildStructuredConfig(toolId, buildCurrentFields()),
        }));
        await invoke<string>("save_shared_config_profiles", {
          name: draftName.trim(),
          profiles: profilesPayload,
          groupKey: editingProfile?.source_type === "shared" ? editingProfile.source_key : null,
          replaceProfileId: editingProfile && editingProfile.source_type !== "shared" ? editingProfile.id : null,
        });
        showToast("success", localeText("共享配置已保存", "Shared provider saved", "共有 Provider を保存しました"));
      } else if (editingProfile) {
        await invoke("update_config_profile", {
          id: editingProfile.id,
          name: draftName.trim(),
          configSnapshot: draftContent,
        });
        showToast("success", locale === "zh" ? "配置已更新" : "Configuration updated");
      } else {
        await invoke("save_config_profile", {
          name: draftName.trim(),
          toolId: draftTool,
          configSnapshot: draftContent,
        });
        showToast("success", locale === "zh" ? "配置已保存" : "Configuration saved");
      }
      closeModal();
      await load();
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `保存失败: ${e}` : `Save failed: ${e}`);
    } finally {
      setSaving(false);
    }
  }, [
    buildCurrentFields,
    closeModal,
    draftContent,
    draftName,
    draftTargetTools,
    draftTool,
    editingProfile,
    load,
    locale,
    localeText,
    saving,
  ]);

  useEffect(() => {
    shortcutFlagsRef.current = {
      canSave: (showCreateModal || !!editingProfile) && draftName.trim().length > 0 && !saving,
      isEditing: showCreateModal || !!editingProfile,
    };
  }, [draftName, editingProfile, saving, showCreateModal]);

  useEffect(() => {
    shortcutHandlersRef.current = {
      save: handleSaveModal,
      create: openCreateModal,
    };
  }, [handleSaveModal, openCreateModal]);

  useEffect(() => {
    const handleSaveShortcut = () => {
      if (shortcutFlagsRef.current.canSave) {
        void shortcutHandlersRef.current.save();
      }
    };
    const handleNewShortcut = () => {
      if (!shortcutFlagsRef.current.isEditing) {
        void shortcutHandlersRef.current.create();
      }
    };
    const handleSearchShortcut = () => {
      if (shortcutFlagsRef.current.isEditing) return;
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };
    window.addEventListener("cchub-shortcut-save", handleSaveShortcut);
    window.addEventListener("cchub-shortcut-new", handleNewShortcut);
    window.addEventListener("cchub-shortcut-search", handleSearchShortcut);
    return () => {
      window.removeEventListener("cchub-shortcut-save", handleSaveShortcut);
      window.removeEventListener("cchub-shortcut-new", handleNewShortcut);
      window.removeEventListener("cchub-shortcut-search", handleSearchShortcut);
    };
  }, []);

  const doApply = useCallback(async (profile: ConfigProfile) => {
    setApplying(profile.id);
    try {
      const result = await applyConfigProfileMutation.mutateAsync(profile.id);
      setActiveIds(result.activeProfileIds);
      setProfiles((current) =>
        current.map((item) =>
          item.id === profile.id
            ? { ...item, updated_at: result.appliedAt }
            : item,
        ),
      );
      showToast("success", locale === "zh" ? "配置已切换" : "Configuration switched");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `切换失败: ${e}` : `Switch failed: ${e}`);
    } finally {
      setApplying(null);
    }
  }, [applyConfigProfileMutation, locale]);


  const handleDelete = useCallback(async (profile: ConfigProfile) => {
    setConfirmAction({ type: "delete", profile });
  }, []);

  const doDelete = useCallback(async (profile: ConfigProfile) => {
    try {
      if (profile.source_type === "shared" && profile.source_key) {
        const removedCount = await invoke<number>("delete_config_profile_group", {
          sourceKey: profile.source_key,
        });
        await load();
        showToast(
          "success",
          localeText(
            `共享配置组已删除（${removedCount} 个 App）`,
            `Shared provider group deleted (${removedCount} apps)`,
            `共有 Provider グループを削除しました（${removedCount} 件の App）`,
          ),
        );
        return;
      }
      if (profile.source_type !== "manual") {
        showToast("error", locale === "zh" ? "当前配置/同步配置不支持删除" : "Live or synced profiles cannot be deleted");
        return;
      }
      await invoke("delete_config_profile", { id: profile.id });
      await load();
      showToast("success", locale === "zh" ? "配置已删除" : "Configuration deleted");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `删除失败: ${e}` : `Delete failed: ${e}`);
    }
  }, [load, locale, localeText]);

  const handleDuplicate = useCallback(async (profile: ConfigProfile) => {
    try {
      const name = profile.name + (locale === "zh" ? " (副本)" : " (Copy)");
      await invoke("save_config_profile", {
        name,
        toolId: profile.tool_id,
        configSnapshot: profile.config_snapshot,
      });
      await load();
      showToast("success", locale === "zh" ? "配置已复制" : "Configuration duplicated");
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `复制失败: ${e}` : `Duplicate failed: ${e}`);
    }
  }, [locale]);

  const handlePing = useCallback(async (profile: ConfigProfile) => {
    setPingingId(profile.id);
    try {
      const result = await invoke<ProviderPingResult>("ping_provider_endpoint", { id: profile.id });
      setPingResults((current) => ({ ...current, [profile.id]: result }));
      if (result.status !== "error") {
        showToast(
          "success",
          locale === "zh"
            ? `已测速 ${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`
            : `Pinged ${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`,
        );
      } else {
        showToast("error", result.message);
      }
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `测速失败: ${e}` : `Ping failed: ${e}`);
    } finally {
      setPingingId((current) => current === profile.id ? null : current);
    }
  }, [locale]);

  const runStreamCheck = useCallback(async (profile: ConfigProfile) => {
    setStreamCheckingId(profile.id);
    try {
      const result = await invoke<ProviderStreamCheckResult>("stream_check_config_profile", { id: profile.id });
      setStreamCheckResults((current) => ({ ...current, [profile.id]: result }));
      if (result.status === "healthy" || result.status === "reachable") {
        showToast(
          "success",
          locale === "zh"
            ? `流式检查完成：${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`
            : `Stream check finished: ${profile.name}${result.latency_ms != null ? ` · ${result.latency_ms}ms` : ""}`,
        );
      } else {
        showToast("error", result.message);
      }
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `流式检查失败: ${e}` : `Stream check failed: ${e}`);
    } finally {
      setStreamCheckingId((current) => current === profile.id ? null : current);
    }
  }, [locale]);

  const handleStreamCheck = useCallback((profile: ConfigProfile) => {
    if (localStorage.getItem("cchub-stream-check-confirmed") === "1") {
      void runStreamCheck(profile);
      return;
    }
    setStreamCheckConfirmProfile(profile);
  }, [runStreamCheck]);

  const reorderProfiles = useCallback(async (sourceId: string, targetId: string) => {
    if (!filterTool || sourceId === targetId || search.trim()) return;
    const orderedProfiles = [...profiles]
      .filter((profile) => profile.tool_id === filterTool)
      .sort((a, b) => {
        const orderDiff = (a.sort_order ?? 0) - (b.sort_order ?? 0);
        if (orderDiff !== 0) return orderDiff;
        const aTime = a.updated_at || a.created_at || "";
        const bTime = b.updated_at || b.created_at || "";
        return bTime.localeCompare(aTime);
      });
    const fromIndex = orderedProfiles.findIndex((profile) => profile.id === sourceId);
    const toIndex = orderedProfiles.findIndex((profile) => profile.id === targetId);
    if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) return;

    const nextOrdered = [...orderedProfiles];
    const [moved] = nextOrdered.splice(fromIndex, 1);
    nextOrdered.splice(toIndex, 0, moved);
    const nextOrderMap = new Map(nextOrdered.map((profile, index) => [profile.id, index]));

    setProfiles((current) =>
      current.map((profile) =>
        profile.tool_id === filterTool && nextOrderMap.has(profile.id)
          ? { ...profile, sort_order: nextOrderMap.get(profile.id) ?? profile.sort_order }
          : profile,
      ),
    );

    try {
      await invoke("reorder_config_profiles", {
        toolId: filterTool,
        orderedIds: nextOrdered.map((profile) => profile.id),
      });
      await invoke("refresh_tray_provider_menu").catch(() => undefined);
    } catch (e) {
      console.error(e);
      showToast("error", locale === "zh" ? `排序失败: ${e}` : `Reorder failed: ${e}`);
      await load();
    } finally {
      setDraggingProfileId(null);
      setDragOverProfileId(null);
    }
  }, [filterTool, load, locale, profiles, search]);

  useEffect(() => { void load(); }, [load]);

  const activeIdSet = useMemo(() => new Set(activeIds), [activeIds]);
  const presetCategories = useMemo(() => getPresetCategories(draftTool), [draftTool]);
  const reorderEnabled = Boolean(filterTool) && search.trim().length === 0;
  const structuredInstalledTools = useMemo(
    () => tools.filter((tool) => tool.installed && supportsStructuredConfig(tool.id)),
    [tools],
  );
  const toolNameMap = useMemo(
    () => Object.fromEntries(tools.map((tool) => [tool.id, tool.name])),
    [tools],
  );
  const handleToggleDraftTargetTool = useCallback((toolId: string) => {
    if (!supportsStructuredConfig(toolId)) return;
    const alreadySelected = draftTargetTools.includes(toolId);
    if (alreadySelected && draftTargetTools.length === 1) {
      return;
    }

    const structuredToolIds = structuredInstalledTools.map((tool) => tool.id);
    const nextTargets = structuredToolIds.filter((id) => {
      if (id === toolId) return !alreadySelected;
      return draftTargetTools.includes(id);
    });

    if (nextTargets.length === 0) {
      return;
    }

    setDraftTargetTools(nextTargets);
    if (!nextTargets.includes(draftTool)) {
      setDraftTool(nextTargets[0]);
      setDraftContent(buildStructuredConfig(nextTargets[0], buildCurrentFields()));
    }
  }, [buildCurrentFields, draftTargetTools, draftTool, structuredInstalledTools]);
  const handleSaveFragment = useCallback(async () => {
    if (!supportsStructuredConfig(draftTool) || savingFragment || !draftFragmentName.trim()) return;
    setSavingFragment(true);
    try {
      const saved = await invoke<ProviderConfigFragment>("save_provider_config_fragment", {
        name: draftFragmentName.trim(),
        targetTools: draftTargetTools.filter((toolId) => supportsStructuredConfig(toolId)),
        fields: buildCurrentFields(),
      });
      setProviderFragments((current) => sortProviderFragments([
        saved,
        ...current.filter((fragment) => fragment.id !== saved.id),
      ]));
      setDraftFragmentName("");
      showToast(
        "success",
        localeText("配置片段已保存", "Provider fragment saved", "Provider フラグメントを保存しました"),
      );
    } catch (e) {
      console.error(e);
      showToast(
        "error",
        localeText(`保存片段失败: ${e}`, `Failed to save fragment: ${e}`, `フラグメントの保存に失敗しました: ${e}`),
      );
    } finally {
      setSavingFragment(false);
    }
  }, [
    buildCurrentFields,
    draftFragmentName,
    draftTargetTools,
    draftTool,
    localeText,
    savingFragment,
  ]);
  const handleApplyFragmentById = useCallback((fragmentId: string) => {
    const fragment = providerFragments.find((item) => item.id === fragmentId);
    if (!fragment) return;
    const includeToolSpecific = fragment.targetTools.includes(draftTool);
    const merged = mergeSharedDraftFields(
      buildCurrentFields(),
      draftTool,
      normalizeFragmentFields(fragment),
      true,
      includeToolSpecific,
    );
    setDraftFields(merged);
    setDraftContent(buildStructuredConfig(draftTool, merged));
    showToast(
      "success",
      localeText("已应用配置片段", "Provider fragment applied", "Provider フラグメントを適用しました"),
    );
  }, [buildCurrentFields, draftTool, localeText, providerFragments]);
  const handleApplyPreset = useCallback((presetId: string) => {
    const preset = presetCategories.flatMap((group) => group.presets).find((item) => item.id === presetId);
    if (!preset) return;
    const next = applyPresetToFields(draftTool, preset.id, buildCurrentFields());
    updateStructuredDraft(draftTool, next);
  }, [buildCurrentFields, draftTool, presetCategories, updateStructuredDraft]);
  const handleSelectDraftOauthAccount = useCallback((accountId: string | null) => {
    updateStructuredDraft(draftTool, { oauthAccountId: accountId || "" });
  }, [draftTool, updateStructuredDraft]);
  const handleRequestFragmentDelete = useCallback((fragmentId: string) => {
    const fragment = providerFragments.find((item) => item.id === fragmentId);
    if (!fragment) return;
    setConfirmFragmentDelete(fragment);
  }, [providerFragments]);
  const handleRefreshProfiles = useCallback(() => {
    void load();
  }, [load]);
  const handleOpenCreateProfile = useCallback(() => {
    void openCreateModal();
  }, [openCreateModal]);
  const handleSearchChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setSearch(event.target.value);
  }, []);
  const handleClearSearch = useCallback(() => {
    setSearch("");
  }, []);
  const handleDraftNameChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setDraftName(event.target.value);
  }, []);
  const handleDraftFragmentNameChange = useCallback((event: ChangeEvent<HTMLInputElement>) => {
    setDraftFragmentName(event.target.value);
  }, []);
  const handleToggleShowApiKey = useCallback(() => {
    setShowApiKey((current) => !current);
  }, []);
  const handleToggleFilterTool = useCallback((toolId: string) => {
    setFilterTool((prev) => prev === toolId ? "" : toolId);
  }, []);
  const handleSaveFragmentClick = useCallback(() => {
    void handleSaveFragment();
  }, [handleSaveFragment]);
  const handleDraftToolChange = useCallback(async (event: ChangeEvent<HTMLSelectElement>) => {
    const toolId = event.target.value;
    setDraftTool(toolId);
    setNewTool(toolId);
    updateDraftFieldsState((current) => ({ ...current, apiFormat: "anthropic" }));
    if (supportsStructuredConfig(toolId)) {
      if (draftTargetTools.length > 1 || editingProfile?.source_type === "shared") {
        if (!draftTargetTools.includes(toolId)) {
          setDraftTargetTools((current) => [...current, toolId]);
        }
        setDraftContent(buildStructuredConfig(toolId, buildCurrentFields()));
      } else {
        resetStructuredDraft(toolId);
        setDraftTargetTools([toolId]);
      }
    } else {
      setDraftContent("");
      setDraftLoading(true);
      try {
        const configContent = await invoke<string>("read_tool_config", { toolId });
        setDraftContent(prettyJson(configContent));
      } catch (error) {
        console.error(error);
      } finally {
        setDraftLoading(false);
      }
    }
  }, [buildCurrentFields, draftTargetTools, editingProfile?.source_type, resetStructuredDraft, updateDraftFieldsState]);
  const profileCardText = useMemo(() => ({
    activeTag: locale === "zh" ? "当前生效" : "Active",
    pingFast: locale === "zh" ? "快速" : "Fast",
    pingMedium: locale === "zh" ? "一般" : "Medium",
    pingSlow: locale === "zh" ? "较慢" : "Slow",
    pingError: locale === "zh" ? "异常" : "Error",
    streamHealthy: locale === "zh" ? "流检通过" : "Stream OK",
    streamReachable: locale === "zh" ? "流检可达" : "Stream Reachable",
    streamUnsupported: locale === "zh" ? "流检暂不支持" : "Stream Unsupported",
    streamUnconfigured: locale === "zh" ? "流检未配置" : "Stream Unconfigured",
    streamError: locale === "zh" ? "流检异常" : "Stream Error",
    dragEnabledTitle: locale === "zh" ? "拖拽调整顺序" : "Drag to reorder",
    dragDisabledTitle: locale === "zh" ? "先选择单个工具并清空搜索后再排序" : "Filter to one tool and clear search to reorder",
    pingTitle: locale === "zh" ? "端点测速" : "Ping endpoint",
    streamTitle: locale === "zh" ? "流式健康检查" : "Stream health check",
    duplicateTitle: locale === "zh" ? "复制" : "Duplicate",
    editTitle: locale === "zh" ? "编辑" : "Edit",
    deleteTitle: locale === "zh" ? "删除" : "Delete",
    activeButton: locale === "zh" ? "已生效" : "Active",
    applyButton: locale === "zh" ? "切换" : "Apply",
  }), [locale]);

  const toolCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const profile of profiles) {
      counts[profile.tool_id] = (counts[profile.tool_id] || 0) + 1;
    }
    return counts;
  }, [profiles]);

  const sharedGroupCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const profile of profiles) {
      if (profile.source_type === "shared" && profile.source_key) {
        counts[profile.source_key] = (counts[profile.source_key] || 0) + 1;
      }
    }
    return counts;
  }, [profiles]);

  const filteredProfiles = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    return [...profiles]
      .filter((profile) => {
        if (filterTool && profile.tool_id !== filterTool) return false;
        if (!keyword) return true;
        return (
          profile.name.toLowerCase().includes(keyword) ||
          profile.tool_id.toLowerCase().includes(keyword) ||
          profile.config_snapshot.toLowerCase().includes(keyword)
        );
      })
      .sort((a, b) => {
        const toolDiff = a.tool_id.localeCompare(b.tool_id);
        if (!filterTool && toolDiff !== 0) return toolDiff;
        const orderDiff = (a.sort_order ?? 0) - (b.sort_order ?? 0);
        if (orderDiff !== 0) return orderDiff;
        const aTime = a.updated_at || a.created_at || "";
        const bTime = b.updated_at || b.created_at || "";
        const timeDiff = bTime.localeCompare(aTime);
        if (timeDiff !== 0) return timeDiff;
        const activeDiff = Number(activeIdSet.has(b.id)) - Number(activeIdSet.has(a.id));
        if (activeDiff !== 0) return activeDiff;
        return a.name.localeCompare(b.name);
      });
  }, [profiles, filterTool, search, activeIdSet]);

  function handleCardDragStart(profileId: string) {
    setDraggingProfileId(profileId);
  }

  function handleCardDragEnter(profileId: string) {
    if (reorderEnabled && draggingProfileId && draggingProfileId !== profileId) {
      setDragOverProfileId(profileId);
    }
  }

  function handleCardDragEnd() {
    setDraggingProfileId(null);
    setDragOverProfileId(null);
  }

  function handleCardDrop(profileId: string) {
    if (!reorderEnabled || !draggingProfileId) return;
    void reorderProfiles(draggingProfileId, profileId);
  }

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {locale === "zh" ? "加载中..." : "Loading..."}
        </span>
      </div>
    );
  }

  const isEditing = showCreateModal || !!editingProfile;
  const isStructured = supportsStructuredConfig(draftTool);

  if (isEditing) {
    return (
      <ProfileEditor
        title={editingProfile ? (locale === "zh" ? "编辑配置" : "Edit Configuration") : (locale === "zh" ? "新增配置" : "New Configuration")}
        subtitle={editingProfile ? (locale === "zh" ? "修改配置名称和参数" : "Update configuration name and parameters") : (locale === "zh" ? "创建一个新的工具配置" : "Create a new tool configuration")}
        onClose={closeModal}
        onSave={() => void handleSaveModal()}
        saveDisabled={!draftName.trim() || saving}
        saving={saving}
      >
          <ProfileBasicInfoSection
            locale={locale}
            localeText={localeText}
            tools={tools}
            draftTool={draftTool}
            draftName={draftName}
            isStructured={isStructured}
            syncTargetsLocked={!!editingProfile && !(draftTargetTools.length > 1 || editingProfile.source_type === "shared")}
            draftTargetTools={draftTargetTools}
            structuredInstalledTools={structuredInstalledTools}
            onToolChange={handleDraftToolChange}
            onNameChange={handleDraftNameChange}
            onToggleDraftTargetTool={handleToggleDraftTargetTool}
          />

          {isStructured && (
            <>
              <ProfilePresetSection
                locale={locale}
                localeText={localeText}
                draftTool={draftTool}
                draftPresetId={draftPresetId}
                presetCategories={presetCategories}
                draftFragmentName={draftFragmentName}
                savingFragment={savingFragment}
                providerFragments={providerFragments}
                toolNameMap={toolNameMap}
                deletingFragmentId={deletingFragmentId}
                onPresetApply={handleApplyPreset}
                onFragmentNameChange={handleDraftFragmentNameChange}
                onSaveFragment={handleSaveFragmentClick}
                onApplyFragment={handleApplyFragmentById}
                onDeleteFragment={handleRequestFragmentDelete}
              />

              <ProfileConnectionSection
                locale={locale}
                localeText={localeText}
                draftTool={draftTool}
                draftProviderType={draftProviderType}
                draftRequiresOAuth={draftRequiresOAuth}
                draftOauthAccountId={draftOauthAccountId}
                showApiKey={showApiKey}
                draftApiKey={draftApiKey}
                draftBaseUrl={draftBaseUrl}
                draftCostMultiplier={draftCostMultiplier}
                draftEndpointCandidates={draftEndpointCandidates}
                draftAuthField={draftAuthField}
                draftApiFormat={draftApiFormat}
                draftCodexReasoningEffort={draftCodexReasoningEffort}
                draftCodexWireApi={draftCodexWireApi}
                draftApiProtocol={draftApiProtocol}
                draftModelCatalogAlias={draftModelCatalogAlias}
                draftNpm={draftNpm}
                draftOpenCodeThinkingLevel={draftOpenCodeThinkingLevel}
                onDraftChange={updateStructuredDraft}
                onAccountSelect={handleSelectDraftOauthAccount}
                onToggleApiKeyVisibility={handleToggleShowApiKey}
              />

              <ProfileModelsSection
                locale={locale}
                draftTool={draftTool}
                draftModel={draftModel}
                draftReasoningModel={draftReasoningModel}
                draftHaikuModel={draftHaikuModel}
                draftSonnetModel={draftSonnetModel}
                draftOpusModel={draftOpusModel}
                draftModelName={draftModelName}
                draftOpenCodeContextLimit={draftOpenCodeContextLimit}
                draftOpenCodeOutputLimit={draftOpenCodeOutputLimit}
                draftOpenCodeInputModalities={draftOpenCodeInputModalities}
                draftOpenCodeOutputModalities={draftOpenCodeOutputModalities}
                onDraftChange={updateStructuredDraft}
              />

              <ProfileRawConfigSection
                locale={locale}
                draftTool={draftTool}
                draftContent={draftContent}
                draftHideAttribution={draftHideAttribution}
                draftEffortHigh={draftEffortHigh}
                draftEnableTeammates={draftEnableTeammates}
                onDraftChange={updateStructuredDraft}
                onContentChange={setDraftContent}
              />
            </>
          )}

          {!isStructured && (
            <ProfilePlainConfigSection
              locale={locale}
              draftTool={draftTool}
              draftContent={draftContent}
              draftLoading={draftLoading}
              onContentChange={setDraftContent}
            />
          )}
      </ProfileEditor>
    );
  }

  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div>
          <h2 className="page-title">{locale === "zh" ? "配置切换" : "Config Profiles"}</h2>
          <p className="page-subtitle">
            {locale === "zh" ? `共 ${profiles.length} 个配置，当前生效 ${activeIds.length} 个` : `${profiles.length} profiles, ${activeIds.length} active`}
          </p>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-secondary btn-sm" onClick={handleRefreshProfiles} style={{ gap: 6 }}>
            <RefreshCw size={14} />
            {locale === "zh" ? "刷新" : "Refresh"}
          </button>
          <button className="btn btn-primary btn-sm" onClick={handleOpenCreateProfile} disabled={installedTools.length === 0} style={{ gap: 6 }}>
            <Plus size={14} />
            {locale === "zh" ? "新增" : "New"}
          </button>
        </div>
      </div>

      <div style={{ display: "flex", gap: 12, marginBottom: 16, alignItems: "center", flexWrap: "wrap" }}>
        <div style={{ position: "relative", flex: 1, minWidth: 240, maxWidth: 360 }}>
          <Search size={14} style={{ position: "absolute", left: 12, top: "50%", transform: "translateY(-50%)", color: "var(--text-muted)" }} />
          <input ref={searchInputRef} className="input" style={{ paddingLeft: 36 }} placeholder={localeText("搜索配置...", "Search...", "設定を検索...")} value={search} onChange={handleSearchChange} />
          {search && (
            <button className="btn btn-ghost btn-icon-sm" style={{ position: "absolute", right: 6, top: "50%", transform: "translateY(-50%)" }} onClick={handleClearSearch}>
              <X size={14} />
            </button>
          )}
        </div>
        <div className="tab-bar" style={{ overflow: "auto", flexShrink: 0 }}>
          {tools.map((tool) => (
            <ProfileToolFilterTab
              key={tool.id}
              toolId={tool.id}
              toolName={tool.name}
              count={toolCounts[tool.id] || 0}
              active={filterTool === tool.id}
              dimmed={!tool.installed && (toolCounts[tool.id] || 0) === 0}
              onToggle={handleToggleFilterTool}
            />
          ))}
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }}>
        {filteredProfiles.length === 0 ? (
          <div className="card empty-state" style={{ flex: 1 }}>
            <div className="empty-icon"><ArrowRightLeft size={28} style={{ color: "var(--text-muted)" }} /></div>
            <p style={{ fontSize: 15, fontWeight: 600, color: "var(--text-secondary)" }}>{locale === "zh" ? "没有可显示的配置" : "No configurations to display"}</p>
            <p style={{ fontSize: 13, color: "var(--text-muted)", marginTop: 8, maxWidth: 320 }}>
              {locale === "zh" ? "点击右上角「新增」保存一份当前配置，之后就可以在这里一键切换。" : "Click \"New\" to save a configuration, then switch here."}
            </p>
          </div>
        ) : (
          filteredProfiles.map((profile) => {
            const Icon = TOOL_ICONS[profile.tool_id] || Monitor;
            const isActive = activeIdSet.has(profile.id);
            const summary = extractConfigSummary(profile.tool_id, profile.config_snapshot);
            const ping = pingResults[profile.id];
            const streamCheck = streamCheckResults[profile.id];
            const sharedCount = profile.source_type === "shared" && profile.source_key
              ? sharedGroupCounts[profile.source_key] || 1
              : 0;
            const pingTone = ping?.status === "fast"
              ? "badge-success"
              : ping?.status === "medium"
                ? "badge-warning"
                : ping?.status === "slow"
                  ? "badge-danger"
                  : ping?.status === "error"
                  ? "badge-danger"
                  : "badge-muted";
            const streamTone = streamCheck?.status === "healthy"
              ? "badge-success"
              : streamCheck?.status === "reachable"
                ? "badge-warning"
                : streamCheck?.status === "unsupported"
                  ? "badge-muted"
                  : streamCheck?.status === "unconfigured"
                                    ? "badge-muted"
                                    : "badge-danger";
            return (
              <ProfileCard
                key={profile.id}
                profile={profile}
                icon={Icon}
                isActive={isActive}
                toolTag={profile.tool_id}
                sharedTag={localeText(`共享 ${sharedCount} App`, `Shared ${sharedCount} apps`, `${sharedCount} App 共有`)}
                baseUrl={summary.baseUrl}
                model={summary.model}
                sharedCount={sharedCount}
                ping={ping}
                pingTone={pingTone}
                streamCheck={streamCheck}
                streamTone={streamTone}
                reorderEnabled={reorderEnabled}
                isDragging={draggingProfileId === profile.id}
                isDragOver={dragOverProfileId === profile.id}
                isPinging={pingingId === profile.id}
                isStreamChecking={streamCheckingId === profile.id}
                isApplying={applying === profile.id}
                text={profileCardText}
                onDragStart={handleCardDragStart}
                onDragEnter={handleCardDragEnter}
                onDragEnd={handleCardDragEnd}
                onDrop={handleCardDrop}
                onPing={handlePing}
                onStreamCheck={handleStreamCheck}
                onApply={doApply}
                onDuplicate={handleDuplicate}
                onEdit={openEditModal}
                onDelete={handleDelete}
              />
            );
          })
        )}
      </div>

      <ConfirmDialog
        isOpen={!!confirmAction}
        title={confirmAction?.profile.source_type === "shared"
          ? localeText("删除共享配置", "Delete Shared Provider", "共有 Provider を削除")
          : (locale === "zh" ? "删除配置" : "Delete Configuration")}
        message={confirmAction?.profile.source_type === "shared" && confirmAction.profile.source_key
          ? localeText(
            `确定删除共享配置「${confirmAction.profile.name}」？这会同时删除 ${sharedGroupCounts[confirmAction.profile.source_key] || 1} 个 App 上的联动配置。`,
            `Delete shared provider "${confirmAction.profile.name}"? This also removes the linked profiles across ${sharedGroupCounts[confirmAction.profile.source_key] || 1} apps.`,
            `共有 Provider「${confirmAction.profile.name}」を削除しますか？ ${sharedGroupCounts[confirmAction.profile.source_key] || 1} 個の App にある連動プロファイルも同時に削除されます。`,
          )
          : (locale === "zh" ? `确定删除配置「${confirmAction?.profile.name}」？此操作不可撤销。` : `Delete "${confirmAction?.profile.name}"? This cannot be undone.`)}
        confirmText={localeText("删除", "Delete", "削除")}
        variant="destructive"
        onConfirm={() => {
          if (confirmAction) void doDelete(confirmAction.profile);
          setConfirmAction(null);
        }}
        onCancel={() => setConfirmAction(null)}
      />
      <ConfirmDialog
        isOpen={!!confirmFragmentDelete}
        title={localeText("删除配置片段", "Delete Provider Fragment", "Provider フラグメントを削除")}
        message={confirmFragmentDelete
          ? localeText(
            `确定删除配置片段「${confirmFragmentDelete.name}」？删除后将无法继续复用这组字段。`,
            `Delete provider fragment "${confirmFragmentDelete.name}"? You will no longer be able to reuse this field set.`,
            `Provider フラグメント「${confirmFragmentDelete.name}」を削除しますか？ このフィールドセットは再利用できなくなります。`,
          )
          : ""}
        confirmText={localeText("删除", "Delete", "削除")}
        variant="destructive"
        onConfirm={() => {
          const fragment = confirmFragmentDelete;
          setConfirmFragmentDelete(null);
          if (!fragment) return;
          void doDeleteFragment(fragment);
        }}
        onCancel={() => setConfirmFragmentDelete(null)}
      />
      <ConfirmDialog
        isOpen={!!streamCheckConfirmProfile}
        title={localeText("流式健康检查", "Stream Health Check", "ストリームヘルスチェック")}
        message={
          localeText(
            "将向 Provider 发送一条最小化的流式请求，用于验证端点是否能成功返回首个流式分片。\n\n首次确认后，后续将直接执行。",
            "CCHub will send a minimal streaming request to verify that this provider endpoint can return the first stream chunk successfully.\n\nAfter you confirm once, future checks will run immediately.",
            "Provider に最小限のストリーミングリクエストを送り、最初のストリームチャンクを正しく返せるか確認します。\n\n一度確認すると、以後はすぐに実行されます。",
          )
        }
        confirmText={localeText("继续检查", "Run Check", "チェックを実行")}
        cancelText={localeText("取消", "Cancel", "キャンセル")}
        variant="info"
        onConfirm={() => {
          const profile = streamCheckConfirmProfile;
          setStreamCheckConfirmProfile(null);
          if (!profile) return;
          localStorage.setItem("cchub-stream-check-confirmed", "1");
          void runStreamCheck(profile);
        }}
        onCancel={() => setStreamCheckConfirmProfile(null)}
      />
    </div>
  );
}
