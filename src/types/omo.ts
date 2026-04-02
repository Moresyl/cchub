export interface OmoAgentDef {
  key: string;
  display: string;
  recommended?: string;
  group: "main" | "sub";
}

export interface OmoCategoryDef {
  key: string;
  display: string;
  recommended?: string;
}

export const OMO_BUILTIN_AGENTS: OmoAgentDef[] = [
  { key: "sisyphus", display: "Sisyphus", recommended: "claude-opus-4-6", group: "main" },
  { key: "hephaestus", display: "Hephaestus", recommended: "gpt-5.4", group: "main" },
  { key: "prometheus", display: "Prometheus", recommended: "claude-opus-4-6", group: "main" },
  { key: "atlas", display: "Atlas", recommended: "kimi-k2.5", group: "main" },
  { key: "oracle", display: "Oracle", recommended: "gpt-5.4", group: "sub" },
  { key: "librarian", display: "Librarian", recommended: "gemini-3-flash", group: "sub" },
  { key: "explore", display: "Explore", recommended: "grok-code-fast-1", group: "sub" },
  { key: "multimodal-looker", display: "Multimodal-Looker", recommended: "kimi-k2.5", group: "sub" },
  { key: "metis", display: "Metis", recommended: "claude-opus-4-6", group: "sub" },
  { key: "momus", display: "Momus", recommended: "gpt-5.4", group: "sub" },
  { key: "sisyphus-junior", display: "Sisyphus-Junior", group: "sub" },
];

export const OMO_BUILTIN_CATEGORIES: OmoCategoryDef[] = [
  { key: "visual-engineering", display: "Visual Engineering", recommended: "gemini-3-pro" },
  { key: "ultrabrain", display: "Ultrabrain", recommended: "gpt-5.4" },
  { key: "deep", display: "Deep", recommended: "gpt-5.4" },
  { key: "artistry", display: "Artistry", recommended: "gemini-3-pro" },
  { key: "quick", display: "Quick", recommended: "claude-haiku-4-5" },
  { key: "unspecified-low", display: "Unspecified Low", recommended: "claude-sonnet-4-6" },
  { key: "unspecified-high", display: "Unspecified High", recommended: "claude-opus-4-6" },
  { key: "writing", display: "Writing", recommended: "gemini-3-flash" },
];

export const OMO_SLIM_BUILTIN_AGENTS: OmoAgentDef[] = [
  { key: "orchestrator", display: "Orchestrator", recommended: "claude-opus-4-6", group: "main" },
  { key: "oracle", display: "Oracle", recommended: "gpt-5.4", group: "sub" },
  { key: "librarian", display: "Librarian", recommended: "gemini-3-flash", group: "sub" },
  { key: "explorer", display: "Explorer", recommended: "grok-code-fast-1", group: "sub" },
  { key: "designer", display: "Designer", recommended: "gemini-3-pro", group: "sub" },
  { key: "fixer", display: "Fixer", recommended: "gpt-5.4", group: "sub" },
];
