/* eslint-disable @typescript-eslint/no-explicit-any */
import ProfileEditor from "../../components/ProfileEditor";
import ProfileTransportSettings from "../../components/ProfileTransportSettings";
import CollapsibleSection from "../../components/CollapsibleSection";
import { type ModelInfo } from "../../components/ModelSelector";
import {
  type ApiFormat,
  type ClaudeAuthField,
  type CodexWireApi,
  type OpenClawApiProtocol,
  type OpenCodeNpmPackage,
  type OpenCodeThinkingLevel,
  type PresetProviderType,
  type StructuredDraftFields,
  getPresetCategories,
} from "../../lib/configProfiles";

import {
  ProfileBasicInfoSection,
  ProfileConnectionSection,
  ProfileModelsSection,
  ProfilePlainConfigSection,
  ProfilePresetSection,
  ProfileRawConfigSection,
} from "./sections";
import type { ConfigProfile, DetectedTool, ProviderConfigFragment } from "./helpers";
import DraftModelTestPanel from "./DraftModelTestPanel";

type LocaleText = (zh: string, en: string, ja?: string) => string;

interface ProfileEditorViewProps {
  draftFields: StructuredDraftFields;
  locale: string;
  localeText: LocaleText;
  editingProfile: ConfigProfile | null;
  closeModal: () => void;
  handleSaveModal: () => Promise<void> | void;
  draftName: string;
  saving: boolean;

  // basic info
  tools: DetectedTool[];
  draftTool: string;
  isStructured: boolean;
  draftTargetTools: string[];
  structuredInstalledTools: DetectedTool[];
  handleDraftToolChange: (value: string) => void;
  handleDraftNameChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  handleToggleDraftTargetTool: (toolId: string) => void;

  // preset
  draftPresetId: string;
  presetCategories: ReturnType<typeof getPresetCategories>;
  draftFragmentName: string;
  savingFragment: boolean;
  providerFragments: ProviderConfigFragment[];
  toolNameMap: Record<string, string>;
  deletingFragmentId: string | null;
  handleApplyPreset: (presetId: string) => void;
  handleDraftFragmentNameChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  handleSaveFragmentClick: () => void;
  handleApplyFragmentById: (fragmentId: string) => void;
  handleRequestFragmentDelete: (fragmentId: string) => void;

  // connection
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
  draftCustomEndpoints: string[];
  draftCustomUserAgent: string;
  draftRequestHeaders: Record<string, string>;
  draftRequestHeaderOverrides: string;
  draftRequestBodyOverrides: string;
  draftAuthField: ClaudeAuthField;
  draftApiFormat: ApiFormat;
  draftCodexWireApi: CodexWireApi;
  draftApiProtocol: OpenClawApiProtocol;
  draftModelCatalogAlias: string;
  draftNpm: OpenCodeNpmPackage;
  draftOpenCodeThinkingLevel: OpenCodeThinkingLevel | "";
  draftHermesProvider: string;
  draftHermesApiKeyEnv: string;
  updateStructuredDraft: (toolId: string, next: Partial<StructuredDraftFields>) => void;
  handleSelectDraftOauthAccount: (accountId: string | null) => void;
  handleToggleShowApiKey: () => void;

  // models
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
  handleFetchModels: () => void;

  // raw/plain config
  draftContent: string;
  draftHideAttribution: boolean;
  draftEffortHigh: boolean;
  draftEnableTeammates: boolean;
  draftLoading: boolean;
  setDraftContent: (value: string) => void;
}

export default function ProfileEditorView(props: ProfileEditorViewProps) {
  const { locale, localeText, editingProfile, closeModal, handleSaveModal, draftName, saving } = props;
  const editorTitle = editingProfile
    ? locale === "zh"
      ? "编辑配置"
      : "Edit Configuration"
    : locale === "zh"
      ? "新增配置"
      : "New Configuration";
  const editorSubtitle = editingProfile
    ? locale === "zh"
      ? "修改配置名称和参数"
      : "Update configuration name and parameters"
    : locale === "zh"
      ? "创建一个新的工具配置"
      : "Create a new tool configuration";
  return (
    <ProfileEditor
      title={editorTitle}
      subtitle={editorSubtitle}
      onClose={closeModal}
      onSave={() => void handleSaveModal()}
      saveDisabled={!draftName.trim() || saving}
      saving={saving}
    >
      <ProfileBasicInfoSection
        locale={locale}
        localeText={localeText}
        tools={props.tools}
        draftTool={props.draftTool}
        draftName={draftName}
        isStructured={props.isStructured}
        syncTargetsLocked={
          !!editingProfile && !(props.draftTargetTools.length > 1 || editingProfile.source_type === "shared")
        }
        draftTargetTools={props.draftTargetTools}
        structuredInstalledTools={props.structuredInstalledTools}
        onToolChange={props.handleDraftToolChange}
        onNameChange={props.handleDraftNameChange}
        onToggleDraftTargetTool={props.handleToggleDraftTargetTool}
      />

      {props.isStructured && (
        <>
          <ProfilePresetSection
            locale={locale}
            localeText={localeText}
            draftTool={props.draftTool}
            draftPresetId={props.draftPresetId}
            presetCategories={props.presetCategories}
            draftFragmentName={props.draftFragmentName}
            savingFragment={props.savingFragment}
            providerFragments={props.providerFragments}
            toolNameMap={props.toolNameMap}
            deletingFragmentId={props.deletingFragmentId}
            onPresetApply={props.handleApplyPreset}
            onFragmentNameChange={props.handleDraftFragmentNameChange}
            onSaveFragment={props.handleSaveFragmentClick}
            onApplyFragment={props.handleApplyFragmentById}
            onDeleteFragment={props.handleRequestFragmentDelete}
          />

          <ProfileConnectionSection
            locale={locale}
            localeText={localeText}
            draftTool={props.draftTool}
            draftProviderType={props.draftProviderType}
            draftRequiresOAuth={props.draftRequiresOAuth}
            draftOauthAccountId={props.draftOauthAccountId}
            showApiKey={props.showApiKey}
            draftApiKey={props.draftApiKey}
            draftBaseUrl={props.draftBaseUrl}
            draftUseFullUrl={props.draftUseFullUrl}
            draftIconUrl={props.draftIconUrl}
            draftCostMultiplier={props.draftCostMultiplier}
            draftEndpointCandidates={props.draftEndpointCandidates}
            draftCustomEndpoints={props.draftCustomEndpoints}
            providerId={editingProfile?.id}
            draftAuthField={props.draftAuthField}
            draftApiFormat={props.draftApiFormat}
            draftCodexWireApi={props.draftCodexWireApi}
            draftApiProtocol={props.draftApiProtocol}
            draftModelCatalogAlias={props.draftModelCatalogAlias}
            draftNpm={props.draftNpm}
            draftOpenCodeThinkingLevel={props.draftOpenCodeThinkingLevel}
            draftHermesProvider={props.draftHermesProvider}
            draftHermesApiKeyEnv={props.draftHermesApiKeyEnv}
            onDraftChange={props.updateStructuredDraft}
            onAccountSelect={props.handleSelectDraftOauthAccount}
            onToggleApiKeyVisibility={props.handleToggleShowApiKey}
          />

          <ProfileModelsSection
            locale={locale}
            localeText={localeText}
            draftTool={props.draftTool}
            draftFields={props.draftFields}
            fetchedModels={props.fetchedModels}
            fetchedModelDetails={props.fetchedModelDetails}
            fetchingModels={props.fetchingModels}
            modelFetchError={props.modelFetchError}
            onFetchModels={props.handleFetchModels}
            onDraftChange={props.updateStructuredDraft}
          />

          <DraftModelTestPanel
            scope={editingProfile?.id ?? "new"}
            toolId={props.draftTool}
            snapshot={props.draftContent}
            model={props.draftModel}
            configuredModels={
              props.draftTool === "claude"
                ? [
                    props.draftModel,
                    props.draftReasoningModel,
                    props.draftHaikuModel,
                    props.draftSonnetModel,
                    props.draftOpusModel,
                  ]
                : [props.draftModel]
            }
            catalog={props.fetchedModelDetails}
            localeText={localeText}
          />

          <ProfileRawConfigSection
            locale={locale}
            draftTool={props.draftTool}
            draftContent={props.draftContent}
            draftHideAttribution={props.draftHideAttribution}
            draftEffortHigh={props.draftEffortHigh}
            draftEnableTeammates={props.draftEnableTeammates}
            onDraftChange={props.updateStructuredDraft}
            onContentChange={props.setDraftContent}
          />

          <CollapsibleSection
            title={localeText("请求设置", "Request settings", "リクエスト設定")}
            summary={localeText("请求头与参数覆盖", "Headers and overrides", "ヘッダーとパラメータ")}
            defaultOpen={
              !!(
                props.draftCustomUserAgent ||
                Object.keys(props.draftRequestHeaders).length ||
                props.draftRequestHeaderOverrides ||
                props.draftRequestBodyOverrides
              )
            }
          >
            <ProfileTransportSettings
              localeText={localeText}
              customUserAgent={props.draftCustomUserAgent}
              requestHeaders={props.draftRequestHeaders}
              requestHeaderOverrides={props.draftRequestHeaderOverrides}
              requestBodyOverrides={props.draftRequestBodyOverrides}
              onChange={(next) => props.updateStructuredDraft(props.draftTool, next)}
            />
          </CollapsibleSection>
        </>
      )}

      {!props.isStructured && (
        <ProfilePlainConfigSection
          locale={locale}
          draftTool={props.draftTool}
          draftContent={props.draftContent}
          draftLoading={props.draftLoading}
          onContentChange={props.setDraftContent}
        />
      )}
    </ProfileEditor>
  );
}
