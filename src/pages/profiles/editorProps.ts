/* eslint-disable @typescript-eslint/no-explicit-any */
import type { ChangeEvent } from "react";

import { type ModelInfo } from "../../components/ModelSelector";
import { type StructuredDraftFields, getPresetCategories } from "../../lib/configProfiles";
import type { ConfigProfile, DetectedTool, ProviderConfigFragment } from "./helpers";

type LocaleText = (zh: string, en: string, ja?: string) => string;

// 编辑视图的 props 多到给 ProfileEditorView 单独包一层，避免 Profiles.tsx 主体被 70+ 行 props 撑死。
export interface BuildEditorPropsInput {
  locale: string;
  localeText: LocaleText;
  editingProfile: ConfigProfile | null;
  closeModal: () => void;
  handleSaveModal: () => Promise<void> | void;
  draftName: string;
  saving: boolean;
  tools: DetectedTool[];
  draftTool: string;
  isStructured: boolean;
  draftTargetTools: string[];
  structuredInstalledTools: DetectedTool[];
  handleDraftToolChange: (event: ChangeEvent<HTMLSelectElement>) => void;
  handleDraftNameChange: (event: ChangeEvent<HTMLInputElement>) => void;
  handleToggleDraftTargetTool: (toolId: string) => void;
  presetCategories: ReturnType<typeof getPresetCategories>;
  draftFragmentName: string;
  savingFragment: boolean;
  providerFragments: ProviderConfigFragment[];
  toolNameMap: Record<string, string>;
  deletingFragmentId: string | null;
  handleApplyPreset: (presetId: string) => void;
  handleDraftFragmentNameChange: (event: ChangeEvent<HTMLInputElement>) => void;
  handleSaveFragmentClick: () => void;
  handleApplyFragmentById: (fragmentId: string) => void;
  handleRequestFragmentDelete: (fragmentId: string) => void;
  draftFields: StructuredDraftFields;
  showApiKey: boolean;
  updateStructuredDraft: (toolId: string, next: Partial<StructuredDraftFields>) => void;
  handleSelectDraftOauthAccount: (accountId: string | null) => void;
  handleToggleShowApiKey: () => void;
  fetchedModels: string[];
  fetchedModelDetails: ModelInfo[];
  fetchingModels: boolean;
  modelFetchError: string | null;
  handleFetchModels: () => void;
  draftContent: string;
  draftLoading: boolean;
  setDraftContent: (value: string) => void;
}

export function buildEditorViewProps(input: BuildEditorPropsInput) {
  const { draftFields } = input;
  return {
    locale: input.locale,
    localeText: input.localeText,
    editingProfile: input.editingProfile,
    closeModal: input.closeModal,
    handleSaveModal: input.handleSaveModal,
    draftName: input.draftName,
    saving: input.saving,
    tools: input.tools,
    draftTool: input.draftTool,
    isStructured: input.isStructured,
    draftTargetTools: input.draftTargetTools,
    structuredInstalledTools: input.structuredInstalledTools,
    handleDraftToolChange: input.handleDraftToolChange,
    handleDraftNameChange: input.handleDraftNameChange,
    handleToggleDraftTargetTool: input.handleToggleDraftTargetTool,
    draftPresetId: draftFields.presetId,
    presetCategories: input.presetCategories,
    draftFragmentName: input.draftFragmentName,
    savingFragment: input.savingFragment,
    providerFragments: input.providerFragments,
    toolNameMap: input.toolNameMap,
    deletingFragmentId: input.deletingFragmentId,
    handleApplyPreset: input.handleApplyPreset,
    handleDraftFragmentNameChange: input.handleDraftFragmentNameChange,
    handleSaveFragmentClick: input.handleSaveFragmentClick,
    handleApplyFragmentById: input.handleApplyFragmentById,
    handleRequestFragmentDelete: input.handleRequestFragmentDelete,
    draftProviderType: draftFields.providerType,
    draftRequiresOAuth: draftFields.requiresOAuth,
    draftOauthAccountId: draftFields.oauthAccountId,
    showApiKey: input.showApiKey,
    draftApiKey: draftFields.apiKey,
    draftBaseUrl: draftFields.baseUrl,
    draftUseFullUrl: draftFields.useFullUrl,
    draftIconUrl: draftFields.iconUrl,
    draftCostMultiplier: draftFields.costMultiplier,
    draftEndpointCandidates: draftFields.endpointCandidates,
    draftCustomEndpoints: draftFields.customEndpoints,
    draftCustomUserAgent: draftFields.customUserAgent,
    draftRequestHeaders: draftFields.requestHeaders,
    draftRequestHeaderOverrides: draftFields.requestHeaderOverrides,
    draftRequestBodyOverrides: draftFields.requestBodyOverrides,
    draftAuthField: draftFields.authField,
    draftApiFormat: draftFields.apiFormat,
    draftCodexReasoningEffort: draftFields.codexReasoningEffort,
    draftCodexWireApi: draftFields.codexWireApi,
    draftApiProtocol: draftFields.apiProtocol,
    draftModelCatalogAlias: draftFields.modelCatalogAlias,
    draftNpm: draftFields.npm,
    draftOpenCodeThinkingLevel: draftFields.openCodeThinkingLevel,
    draftHermesProvider: draftFields.hermesProvider,
    draftHermesApiKeyEnv: draftFields.hermesApiKeyEnv,
    updateStructuredDraft: input.updateStructuredDraft,
    handleSelectDraftOauthAccount: input.handleSelectDraftOauthAccount,
    handleToggleShowApiKey: input.handleToggleShowApiKey,
    draftModel: draftFields.model,
    draftReasoningModel: draftFields.reasoningModel,
    draftHaikuModel: draftFields.haikuModel,
    draftSonnetModel: draftFields.sonnetModel,
    draftOpusModel: draftFields.opusModel,
    draftModelName: draftFields.modelName,
    draftOpenCodeContextLimit: draftFields.openCodeContextLimit,
    draftOpenCodeOutputLimit: draftFields.openCodeOutputLimit,
    draftOpenCodeInputModalities: draftFields.openCodeInputModalities,
    draftOpenCodeOutputModalities: draftFields.openCodeOutputModalities,
    fetchedModels: input.fetchedModels,
    fetchedModelDetails: input.fetchedModelDetails,
    fetchingModels: input.fetchingModels,
    modelFetchError: input.modelFetchError,
    handleFetchModels: input.handleFetchModels,
    draftContent: input.draftContent,
    draftHideAttribution: draftFields.hideAttribution,
    draftEffortHigh: draftFields.effortHigh,
    draftEnableTeammates: draftFields.enableTeammates,
    draftLoading: input.draftLoading,
    setDraftContent: input.setDraftContent,
  };
}
