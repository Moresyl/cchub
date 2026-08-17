import { invoke } from "@tauri-apps/api/core";

export interface FetchedModel {
  id: string;
  ownedBy: string | null;
}

export interface ModelFetchOptions {
  apiFormat?: string;
  requestHeaders?: Record<string, string>;
}

export type PiPromptFileKind = "system_override" | "system_append";

export interface PiPromptFileSnapshot {
  exists: boolean;
  revision: string;
  content: string;
}

export interface PiPromptTemplate {
  slug: string;
  content: string;
  revision: string;
}

export interface PiCurrentState {
  enabledProviderIds: string[];
  defaultProviderId: string | null;
}

export type PiSessionDiscovery =
  | { status: "available" }
  | { status: "requires_project_context"; configuredPath: string }
  | { status: "unavailable"; reason: string };

export interface PiUsageScript {
  enabled: boolean;
  language: string;
  code: string;
  timeout?: number;
  apiKey?: string;
  baseUrl?: string;
  accessToken?: string;
  userId?: string;
  templateType?: string;
  autoQueryInterval?: number;
  codingPlanProvider?: string;
  accessKeyId?: string;
  secretAccessKey?: string;
  teamOrganizationId?: string;
  teamProjectId?: string;
}

export interface SessionSyncResult {
  imported: number;
  skipped: number;
  filesScanned: number;
  suspectedDuplicates: number;
  deferredFiles: number;
  errors: string[];
}

export type JsonObject = Record<string, unknown>;

export type SkillPayload = JsonObject;
export type SkillImportPayload = JsonObject;
export type SkillMigrationTarget = string | JsonObject;

/** Backend compatibility commands exposed through a typed frontend boundary. */
export const compatApi = {
  async getPiCurrentState(): Promise<PiCurrentState> {
    return invoke("get_pi_current_state");
  },

  async getPiSessionDiscovery(): Promise<PiSessionDiscovery> {
    return invoke("get_pi_session_discovery");
  },

  async updatePiProviderUsageScript(id: string, usageScript: PiUsageScript): Promise<boolean> {
    return invoke("update_pi_provider_usage_script", { id, usageScript });
  },

  async fetchModelsForConfig(
    baseUrl: string,
    apiKey: string,
    isFullUrl?: boolean,
    modelsUrl?: string,
    customUserAgent?: string,
    options?: ModelFetchOptions,
  ): Promise<FetchedModel[]> {
    return invoke("fetch_models_for_config", {
      baseUrl,
      apiKey,
      isFullUrl,
      modelsUrl,
      customUserAgent,
      apiFormat: options?.apiFormat,
      requestHeaders: options?.requestHeaders,
    });
  },

  async getPiPromptFile(kind: PiPromptFileKind): Promise<PiPromptFileSnapshot> {
    return invoke("get_pi_prompt_file", { kind });
  },

  async replacePiPromptFile(
    kind: PiPromptFileKind,
    expectedRevision: string,
    content: string,
  ): Promise<PiPromptFileSnapshot> {
    return invoke("replace_pi_prompt_file", { kind, expectedRevision, content });
  },

  async deletePiPromptFile(kind: PiPromptFileKind, expectedRevision: string): Promise<boolean> {
    return invoke("delete_pi_prompt_file", { kind, expectedRevision });
  },

  async listPiPromptTemplates(): Promise<PiPromptTemplate[]> {
    return invoke("list_pi_prompt_templates");
  },

  async upsertPiPromptTemplate(
    slug: string,
    expectedRevision: string,
    content: string,
    originalSlug?: string,
  ): Promise<PiPromptTemplate> {
    return invoke("upsert_pi_prompt_template", {
      slug,
      originalSlug: originalSlug ?? null,
      expectedRevision,
      content,
    });
  },

  async deletePiPromptTemplate(slug: string, expectedRevision: string): Promise<boolean> {
    return invoke("delete_pi_prompt_template", { slug, expectedRevision });
  },

  async installSkillUnified(skill: SkillPayload, currentApp: string): Promise<SkillPayload> {
    return invoke("install_skill_unified", { skill, currentApp });
  },

  async scanUnmanagedSkills(): Promise<SkillPayload[]> {
    return invoke("scan_unmanaged_skills");
  },

  async importSkillsFromApps(imports: SkillImportPayload[]): Promise<SkillPayload[]> {
    return invoke("import_skills_from_apps", { imports });
  },

  async migrateSkillStorage(target: SkillMigrationTarget): Promise<SkillPayload> {
    return invoke("migrate_skill_storage", { target });
  },

  async syncSessionUsage(): Promise<SessionSyncResult> {
    return invoke("sync_session_usage");
  },

  async rebuildCodexUsage(): Promise<SessionSyncResult> {
    return invoke("rebuild_codex_usage");
  },

  async ensureGrokbuildOfficialProvider(): Promise<boolean> {
    return invoke("ensure_grokbuild_official_provider");
  },
};
