export type PromptApp = "claude" | "codex" | "gemini" | "opencode" | "openclaw" | "hermes" | "pi";
export interface PromptRecord {
  id: string;
  name: string;
  content: string;
  description?: string | null;
  enabled: boolean;
  createdAt: number;
  updatedAt: number;
}
export interface LibrarySnapshot {
  prompts: Record<string, PromptRecord>;
  libraryRevision: string;
  live: { content: string | null; revision: string } | null;
  liveError: string | null;
}
export interface PromptDraft {
  id: string;
  name: string;
  content: string;
  description: string;
  enabled: boolean;
}
export type Text = (zh: string, en: string, ja?: string) => string;
export const APP_OPTIONS: Array<{ id: PromptApp; label: string; file: string }> = [
  { id: "claude", label: "Claude", file: "~/.claude/CLAUDE.md" },
  { id: "codex", label: "Codex", file: "~/.codex/AGENTS.md" },
  { id: "gemini", label: "Gemini", file: "~/.gemini/GEMINI.md" },
  { id: "opencode", label: "OpenCode", file: "~/.config/opencode/AGENTS.md" },
  { id: "openclaw", label: "OpenClaw", file: "~/.openclaw/AGENTS.md" },
  { id: "hermes", label: "Hermes", file: "~/.hermes/SOUL.md" },
  { id: "pi", label: "Pi", file: "~/.pi/agent/AGENTS.md" },
];
export function emptyDraft(): PromptDraft {
  return { id: crypto.randomUUID(), name: "", description: "", content: "", enabled: false };
}
export function draftFrom(record: PromptRecord): PromptDraft {
  return { ...record, description: record.description ?? "" };
}
export function invalidDraft(draft: PromptDraft) {
  return (
    !draft.name.trim() ||
    [...draft.name.trim()].length > 120 ||
    [...draft.description].length > 2000 ||
    new TextEncoder().encode(draft.content).length > 1024 * 1024
  );
}
export function parseSnapshot(value: LibrarySnapshot): LibrarySnapshot {
  if (
    !value ||
    typeof value.libraryRevision !== "string" ||
    !value.libraryRevision ||
    !value.prompts ||
    typeof value.prompts !== "object" ||
    Array.isArray(value.prompts) ||
    Object.entries(value.prompts).some(
      ([id, record]) =>
        !record ||
        record.id !== id ||
        typeof record.name !== "string" ||
        typeof record.content !== "string" ||
        (record.description != null && typeof record.description !== "string") ||
        typeof record.enabled !== "boolean" ||
        !Number.isFinite(record.updatedAt) ||
        !Number.isFinite(record.createdAt),
    ) ||
    (value.live !== null &&
      (!value.live ||
        typeof value.live.revision !== "string" ||
        (value.live.content !== null && typeof value.live.content !== "string"))) ||
    (value.liveError !== null && typeof value.liveError !== "string")
  ) {
    throw new Error("Cannot read prompt library snapshot; reload before writing");
  }
  return value;
}
