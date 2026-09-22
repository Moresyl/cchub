export type McodeApi = "anthropic-messages" | "openai-completions" | "openai-responses";

export interface McodeProvider {
  kind?: string;
  enabled?: boolean;
  api?: McodeApi;
  options?: { baseURL?: string; apiKey?: string; [key: string]: unknown };
  models?: Record<string, { name?: string; [key: string]: unknown }>;
  [key: string]: unknown;
}

export interface McodeState {
  configPath: string;
  installed: boolean;
  providers: Record<string, McodeProvider>;
}

export interface McodeDraft {
  id: string;
  api: McodeApi;
  baseUrl: string;
  apiKey: string;
  models: string;
  enabled: boolean;
}

export function createMcodeDraft(id = "", provider?: McodeProvider): McodeDraft {
  return {
    id,
    api: provider?.api ?? "anthropic-messages",
    baseUrl: provider?.options?.baseURL ?? "",
    apiKey: provider?.options?.apiKey ?? "",
    models: Object.keys(provider?.models ?? {}).join("\n"),
    enabled: provider?.enabled !== false,
  };
}

export function buildMcodeProvider(draft: McodeDraft, existing?: McodeProvider): McodeProvider {
  const ids = Array.from(
    new Set(
      draft.models
        .split(/[\n,]/)
        .map((value) => value.trim())
        .filter(Boolean),
    ),
  );
  return {
    ...existing,
    kind: "custom",
    enabled: draft.enabled,
    api: draft.api,
    options: { ...existing?.options, baseURL: draft.baseUrl.trim(), apiKey: draft.apiKey.trim() },
    models: Object.fromEntries(ids.map((id) => [id, existing?.models?.[id] ?? { name: id }])),
  };
}
