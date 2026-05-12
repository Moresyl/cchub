/* eslint-disable @typescript-eslint/no-explicit-any */
import { memo } from "react";
import { Eye, EyeOff } from "lucide-react";

import CopilotAuthSection from "../../components/CopilotAuthSection";
import {
  type ApiFormat,
  type ClaudeAuthField,
  type CodexReasoningEffort,
  type CodexWireApi,
  type OpenClawApiProtocol,
  type OpenCodeNpmPackage,
  type OpenCodeThinkingLevel,
  type PresetProviderType,
  type StructuredDraftFields,
} from "../../lib/configProfiles";
import {
  CODEX_REASONING_OPTIONS,
  CODEX_WIRE_API_OPTIONS,
  FIELD_STACK_STYLE,
  OPENCLAW_PROTOCOL_OPTIONS,
  OPENCODE_NPM_OPTIONS,
  SECTION_TITLE_STYLE,
  SMALL_INPUT_STYLE,
  THINKING_LEVEL_OPTIONS,
  TWO_COLUMN_GRID_STYLE,
} from "./helpers";

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
  draftUseFullUrl: boolean;
  draftIconUrl: string;
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
  draftHermesProvider: string;
  draftHermesApiKeyEnv: string;
  onDraftChange: (toolId: string, next: Partial<StructuredDraftFields>) => void;
  onAccountSelect: (accountId: string | null) => void;
  onToggleApiKeyVisibility: () => void;
}

const SectionTitle = memo(function SectionTitleFn({ children }: { children: React.ReactNode }) {
  return <h3 style={SECTION_TITLE_STYLE}>{children}</h3>;
});

function Field({ label, children }: { label: string; children: React.ReactNode }) {
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

const SelectField = memo(function SelectFieldFn({
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
        <option key={option} value={option}>
          {option}
        </option>
      ))}
    </select>
  );
});

export const ProfileConnectionSection = memo(function ProfileConnectionSection({
  locale,
  localeText,
  draftTool,
  draftProviderType,
  draftRequiresOAuth,
  draftOauthAccountId,
  showApiKey,
  draftApiKey,
  draftBaseUrl,
  draftUseFullUrl,
  draftIconUrl,
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
  draftHermesProvider,
  draftHermesApiKeyEnv,
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
              placeholder={draftUseFullUrl ? "https://proxy.example.com/v1/messages" : "https://api.example.com"}
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

        <Field label={localeText("图标 URL", "Icon URL", "アイコン URL")}>
          <TextInput
            value={draftIconUrl}
            onChange={(event) => onDraftChange(draftTool, { iconUrl: event.target.value })}
            placeholder="https://example.com/provider-icon.png"
          />
        </Field>
        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: -8 }}>
          {localeText(
            "优先显示这里配置的远程图标；加载失败时会回退到工具默认图标。",
            "This remote icon is preferred when present. If it fails to load, the default tool icon is used.",
            "ここに設定したリモートアイコンを優先表示し、読み込みに失敗した場合は既定のツールアイコンへ戻します。",
          )}
        </div>

        <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
          <input
            type="checkbox"
            checked={draftUseFullUrl}
            onChange={(event) => onDraftChange(draftTool, { useFullUrl: event.target.checked })}
          />
          <span>
            {localeText(
              "将接口地址作为完整端点使用",
              "Use base URL as the full endpoint",
              "Base URL を完全なエンドポイントとして扱う",
            )}
          </span>
        </label>
        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: -8 }}>
          {draftUseFullUrl
            ? localeText(
                "已启用完整端点模式，后端不会再自动补 `/v1/messages`、`/chat/completions` 等路径。",
                "Full endpoint mode is enabled. The backend will stop appending paths such as `/v1/messages` or `/chat/completions`.",
                "完全なエンドポイントモードが有効です。バックエンドは `/v1/messages` や `/chat/completions` などのパスを自動付与しません。",
              )
            : localeText(
                "默认会按供应商协议自动补全标准路径。",
                "Standard provider paths will be appended automatically by default.",
                "デフォルトではプロバイダー規約に従って標準パスが自動補完されます。",
              )}
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

        {draftTool === "hermes" && (
          <div style={TWO_COLUMN_GRID_STYLE}>
            <Field label={locale === "zh" ? "Hermes Provider" : "Hermes Provider"}>
              <SelectField
                value={draftHermesProvider}
                onChange={(value) => onDraftChange(draftTool, { hermesProvider: value })}
                options={["nous", "openrouter", "gemini", "zai", "kimi-coding", "anthropic", "custom"]}
              />
            </Field>
            <Field label={locale === "zh" ? "API Key 环境变量" : "API Key Env"}>
              <TextInput
                value={draftHermesApiKeyEnv}
                onChange={(event) => onDraftChange(draftTool, { hermesApiKeyEnv: event.target.value })}
                placeholder="OPENROUTER_API_KEY"
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
                onChange={(event) =>
                  onDraftChange(draftTool, {
                    openCodeThinkingLevel: event.target.value as OpenCodeThinkingLevel | "",
                  })
                }
                style={SMALL_INPUT_STYLE}
              >
                <option value="">{locale === "zh" ? "无" : "None"}</option>
                {THINKING_LEVEL_OPTIONS.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </Field>
          </div>
        )}
      </div>
    </div>
  );
});

export default ProfileConnectionSection;
