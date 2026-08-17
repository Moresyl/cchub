/* eslint-disable @typescript-eslint/no-explicit-any, react-hooks/rules-of-hooks */
import { memo, lazy, useCallback, type ChangeEvent, type ReactNode } from "react";
import { RefreshCw, Save } from "lucide-react";

import ProfileFragmentCard from "../../components/ProfileFragmentCard";
import ProfilePresetButton from "../../components/ProfilePresetButton";
import ProfileTargetToolToggle from "../../components/ProfileTargetToolToggle";
import ModelSelector, { type ModelInfo } from "../../components/ModelSelector";
import LoadingState from "../../components/states/LoadingState";
import { getPresetCategories, type StructuredDraftFields } from "../../lib/configProfiles";
import {
  FIELD_STACK_STYLE,
  SECTION_TITLE_STYLE,
  SMALL_INPUT_STYLE,
  TWO_COLUMN_GRID_STYLE,
  formatTime,
  getConfigLanguage,
  supportsModelFetch,
  type DetectedTool,
  type ProviderConfigFragment,
} from "./helpers";

const CodeEditor = lazy(() => import("../../components/CodeEditor"));

export const CodexRawConfigEditor = memo(function CodexRawConfigEditor({
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

  const rebuild = useCallback(
    (nextAuth: string, nextToml: string) => {
      try {
        const auth = JSON.parse(nextAuth);
        onChange(JSON.stringify({ ...JSON.parse(value), auth, config: nextToml }, null, 2));
      } catch (error) {
        console.debug("Skipping Codex config rebuild because auth JSON is invalid", error);
      }
    },
    [onChange, value],
  );

  const handleAuthChange = useCallback(
    (nextAuth: string) => {
      rebuild(nextAuth, configToml);
    },
    [configToml, rebuild],
  );

  const handleTomlChange = useCallback(
    (nextToml: string) => {
      rebuild(authJson, nextToml);
    },
    [authJson, rebuild],
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      <div>
        <label className="field-label" style={{ marginBottom: 6 }}>
          auth.json
        </label>
        <CodeEditor value={authJson} onChange={handleAuthChange} language="json" minHeight={80} />
      </div>
      <div>
        <label className="field-label" style={{ marginBottom: 6 }}>
          config.toml
        </label>
        <CodeEditor value={configToml} onChange={handleTomlChange} language="toml" minHeight={200} />
      </div>
    </div>
  );
});

const SectionTitle = memo(function SectionTitle({ children }: { children: ReactNode }) {
  return <h3 style={SECTION_TITLE_STYLE}>{children}</h3>;
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

// SelectField primitive used to live here; ConnectionSection.tsx now embeds its own copy.

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

export const ProfileBasicInfoSection = memo(function ProfileBasicInfoSection({
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
              <option key={tool.id} value={tool.id}>
                {tool.name}
              </option>
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

export const ProfilePresetSection = memo(function ProfilePresetSection({
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
                placeholder={localeText(
                  "例如：OpenAI 兼容基础参数",
                  "e.g. OpenAI-compatible defaults",
                  "例: OpenAI 互換の基本設定",
                )}
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
                compatibilityReadyLabel={localeText(
                  "含当前工具字段",
                  "Includes current tool fields",
                  "現在のツール向け字段あり",
                )}
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

interface ProfileModelsSectionProps {
  locale: string;
  localeText: (zh: string, en: string, ja?: string) => string;
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
  fetchedModels: string[];
  fetchedModelDetails: ModelInfo[];
  fetchingModels: boolean;
  modelFetchError: string | null;
  onFetchModels: () => void;
  onDraftChange: (toolId: string, next: Partial<StructuredDraftFields>) => void;
}

export const ProfileModelsSection = memo(function ProfileModelsSection({
  locale,
  localeText,
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
  fetchedModels,
  fetchedModelDetails,
  fetchingModels,
  modelFetchError,
  onFetchModels,
  onDraftChange,
}: ProfileModelsSectionProps) {
  const canFetchModels = supportsModelFetch(draftTool);
  const hasDetails = fetchedModelDetails.length > 0;

  const ModelInput = useCallback(
    ({
      value,
      onChange: onValueChange,
      placeholder,
    }: {
      value: string;
      onChange: (v: string) => void;
      placeholder?: string;
    }) => {
      if (hasDetails) {
        return (
          <ModelSelector
            value={value}
            models={fetchedModelDetails}
            onChange={onValueChange}
            placeholder={placeholder}
          />
        );
      }
      return <TextInput value={value} onChange={(e) => onValueChange(e.target.value)} placeholder={placeholder} />;
    },
    [hasDetails, fetchedModelDetails],
  );

  return (
    <div>
      <div
        style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 12 }}
      >
        <SectionTitle>{locale === "zh" ? "模型配置" : "Models"}</SectionTitle>
        {canFetchModels && (
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={onFetchModels}
            disabled={fetchingModels}
            style={{ gap: 6, whiteSpace: "nowrap" }}
          >
            {fetchingModels ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <RefreshCw size={14} />}
            {localeText("拉取模型列表", "Fetch Models", "モデル一覧を取得")}
          </button>
        )}
      </div>
      {(modelFetchError || fetchedModels.length > 0) && (
        <div style={{ fontSize: 12, color: modelFetchError ? "var(--danger)" : "var(--text-muted)", marginBottom: 12 }}>
          {modelFetchError ||
            localeText(
              `已发现 ${fetchedModels.length} 个模型`,
              `Discovered ${fetchedModels.length} models`,
              `${fetchedModels.length} 個のモデルを検出しました`,
            )}
        </div>
      )}
      {draftTool === "claude" ? (
        <div style={TWO_COLUMN_GRID_STYLE}>
          <Field label={locale === "zh" ? "主模型" : "Main Model"}>
            <ModelInput
              value={draftModel}
              onChange={(v) => onDraftChange(draftTool, { model: v })}
              placeholder="claude-sonnet-4-5"
            />
          </Field>
          <Field label={locale === "zh" ? "推理模型" : "Reasoning Model"}>
            <ModelInput
              value={draftReasoningModel}
              onChange={(v) => onDraftChange(draftTool, { reasoningModel: v })}
              placeholder="claude-sonnet-4-5"
            />
          </Field>
          <Field label={locale === "zh" ? "Haiku 默认模型" : "Default Haiku"}>
            <ModelInput
              value={draftHaikuModel}
              onChange={(v) => onDraftChange(draftTool, { haikuModel: v })}
              placeholder="claude-haiku-3-5"
            />
          </Field>
          <Field label={locale === "zh" ? "Sonnet 默认模型" : "Default Sonnet"}>
            <ModelInput
              value={draftSonnetModel}
              onChange={(v) => onDraftChange(draftTool, { sonnetModel: v })}
              placeholder="claude-sonnet-4-5"
            />
          </Field>
          <Field label={locale === "zh" ? "Opus 默认模型" : "Default Opus"}>
            <ModelInput
              value={draftOpusModel}
              onChange={(v) => onDraftChange(draftTool, { opusModel: v })}
              placeholder="claude-opus-5"
            />
          </Field>
        </div>
      ) : (
        <div style={TWO_COLUMN_GRID_STYLE}>
          <Field label={locale === "zh" ? "模型 ID" : "Model ID"}>
            <ModelInput
              value={draftModel}
              onChange={(v) => onDraftChange(draftTool, { model: v })}
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
                <TextInput
                  value={draftOpenCodeContextLimit}
                  onChange={(event) => onDraftChange(draftTool, { openCodeContextLimit: event.target.value })}
                  placeholder="400000"
                />
              </Field>
              <Field label={locale === "zh" ? "Output Limit" : "Output Limit"}>
                <TextInput
                  value={draftOpenCodeOutputLimit}
                  onChange={(event) => onDraftChange(draftTool, { openCodeOutputLimit: event.target.value })}
                  placeholder="128000"
                />
              </Field>
              <Field label={locale === "zh" ? "输入模态" : "Input Modalities"}>
                <TextInput
                  value={draftOpenCodeInputModalities}
                  onChange={(event) => onDraftChange(draftTool, { openCodeInputModalities: event.target.value })}
                  placeholder="text,image,pdf"
                />
              </Field>
              <Field label={locale === "zh" ? "输出模态" : "Output Modalities"}>
                <TextInput
                  value={draftOpenCodeOutputModalities}
                  onChange={(event) => onDraftChange(draftTool, { openCodeOutputModalities: event.target.value })}
                  placeholder="text"
                />
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

export const ProfileRawConfigSection = memo(function ProfileRawConfigSection({
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
            <input
              type="checkbox"
              checked={draftHideAttribution}
              onChange={(event) => onDraftChange(draftTool, { hideAttribution: event.target.checked })}
            />
            {locale === "zh" ? "隐藏 AI 署名" : "Hide AI Attribution"}
          </label>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <input
              type="checkbox"
              checked={draftEffortHigh}
              onChange={(event) => onDraftChange(draftTool, { effortHigh: event.target.checked })}
            />
            {locale === "zh" ? "高强度思考" : "High Effort Thinking"}
          </label>
          <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13 }}>
            <input
              type="checkbox"
              checked={draftEnableTeammates}
              onChange={(event) => onDraftChange(draftTool, { enableTeammates: event.target.checked })}
            />
            {locale === "zh" ? "Teammates 模式" : "Teammates Mode"}
          </label>
        </div>
      )}
      <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 10 }}>
        {locale === "zh"
          ? "上方表单字段会自动同步到此处，你也可以直接编辑原始配置。"
          : "Form fields above are synced here. You can also edit the raw config directly."}
      </div>
      {draftTool === "codex" ? (
        <CodexRawConfigEditor value={draftContent} onChange={onContentChange} />
      ) : (
        <CodeEditor
          value={draftContent}
          onChange={onContentChange}
          language={getConfigLanguage(draftTool, draftContent)}
          minHeight={240}
        />
      )}
    </div>
  );
});

export const ProfilePlainConfigSection = memo(function ProfilePlainConfigSection({
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
        <LoadingState />
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

// ConnectionSection 抽到独立文件以保持本文件 < 900 行
export { default as ProfileConnectionSection } from "./ConnectionSection";
