// configProfiles 入口：保留对原 src/lib/configProfiles.ts 的所有 public 导出，
// 仅把内部按 (types / presets/* / helpers / builder / parser) 拆分到独立文件。

export type {
  ApiFormat,
  ClaudeAuthField,
  CodexReasoningEffort,
  CodexWireApi,
  ConfigPreset,
  ModelCost,
  OpenClawApiProtocol,
  OpenClawModelCatalogEntry,
  OpenClawSuggestedDefaults,
  OpenCodeNpmPackage,
  OpenCodeReasoningEffort,
  OpenCodeThinkingLevel,
  PresetProviderType,
  StructuredConfigTool,
  StructuredDraftFields,
  TemplateValueConfig,
} from "./types";

export { PRESETS } from "./presets";

export {
  applyPresetToFields,
  createDefaultStructuredFields,
  findTomlValue,
  getConfigPresets,
  getPresetCategories,
  normalizeCustomUserAgent,
  normalizeEndpointList,
  normalizeRequestHeaders,
  supportsStructuredConfig,
} from "./helpers";

export { buildStructuredConfig } from "./builder";
export { parseStructuredConfig } from "./parser";
