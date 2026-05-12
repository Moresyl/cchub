import type { ManagedAppId } from "../../lib/appPreferences";

export interface SessionSummary {
  id: string;
  tool_id: string;
  tool_name: string;
  title: string;
  cwd: string | null;
  source_kind: string;
  source_backend: string;
  source_path: string;
  created_at: string | null;
  updated_at: string | null;
  preview: string;
  message_count: number;
  input_tokens: number | null;
  output_tokens: number | null;
  tokens_used: number | null;
  search_hit_count: number;
  can_resume: boolean;
  can_delete: boolean;
}

export interface SessionEntry {
  id: string;
  kind: string;
  title: string;
  content: string;
  timestamp: string | null;
}

export interface SessionDetail {
  session: SessionSummary;
  entries: SessionEntry[];
}

export interface SessionDeleteTarget {
  tool_id: string;
  session_id: string;
  source_path: string;
  source_backend: string;
}

export const TOOL_ORDER: ManagedAppId[] = ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"];

export function sessionSelectionKey(session: Pick<SessionSummary, "tool_id" | "id" | "source_path">) {
  return `${session.tool_id}::${session.id}::${session.source_path}`;
}

export function buildSessionDeleteTarget(
  session: Pick<SessionSummary, "tool_id" | "id" | "source_path" | "source_backend">,
): SessionDeleteTarget {
  return {
    tool_id: session.tool_id,
    session_id: session.id,
    source_path: session.source_path,
    source_backend: session.source_backend,
  };
}

export function formatTokenCount(value: number) {
  return new Intl.NumberFormat("en-US").format(value);
}

export function countSessionHits(session: SessionSummary, query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return 0;

  return [
    session.title,
    session.preview,
    session.cwd ?? "",
    session.tool_name,
    session.source_backend,
    session.source_path,
  ].filter((value) => value.toLowerCase().includes(normalized)).length;
}

export function matchesEntry(entry: SessionEntry, query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  return (
    entry.title.toLowerCase().includes(normalized) ||
    entry.kind.toLowerCase().includes(normalized) ||
    entry.content.toLowerCase().includes(normalized)
  );
}

/** Build the CLI resume command for a given tool + session ID. */
export function buildResumeCommand(toolId: string, sessionId: string): string | null {
  switch (toolId) {
    case "claude":
      return `claude --resume ${sessionId}`;
    case "codex":
      return `codex resume ${sessionId}`;
    case "gemini":
      return `gemini --resume ${sessionId}`;
    case "opencode":
      return `opencode session resume ${sessionId}`;
    default:
      return null; // openclaw etc. — no CLI resume
  }
}

export function buildSessionListLabels(locale: string) {
  const text = (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en);
  return {
    copyLabel: text("复制恢复命令", "Copy resume command", "復元コマンドをコピー"),
    copyTitle: text("复制恢复命令", "Copy resume command", "復元コマンドをコピー"),
    deleteTitle: text("删除会话", "Delete session", "会話を削除"),
    deleteLabel: text("删除会话", "Delete session", "会話を削除"),
    selectLabel: text("选择会话", "Select session", "会話を選択"),
    unknownTimeLabel: text("未知时间", "Unknown time", "時刻不明"),
    tokenLabel: (count: number) => `${formatTokenCount(count)} tokens`,
    matchLabel: (count: number) => text(`${count} 处匹配`, `${count} match(es)`, `${count} 件一致`),
    itemsLabel: (count: number) => text(`${count} 条记录`, `${count} items`, `${count} 件`),
  };
}

export function entryBadgeColor(kind: string) {
  switch (kind) {
    case "user":
      return "badge-accent";
    case "assistant":
      return "badge-success";
    case "tool_call":
      return "badge-muted";
    case "tool_output":
      return "badge-muted";
    case "reasoning":
      return "badge-accent";
    default:
      return "badge-muted";
  }
}
