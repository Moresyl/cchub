import type { SessionDetail, SessionSummary } from "./helpers";

export function sessionFixture(overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
    id: "shared-id",
    tool_id: "codex",
    tool_name: "Codex",
    title: "Original title",
    cwd: "C:/fixture/project",
    source_kind: "jsonl",
    source_backend: "file",
    source_path: "C:/fixture/one.jsonl",
    created_at: null,
    updated_at: null,
    preview: "Fixture conversation",
    message_count: 2,
    input_tokens: 10,
    output_tokens: 5,
    tokens_used: 15,
    search_hit_count: 0,
    can_resume: true,
    can_delete: true,
    ...overrides,
  };
}

export function detailFixture(session: SessionSummary): SessionDetail {
  return {
    session,
    entries: [{ id: "entry", kind: "user", title: "User", content: "Fixture question", timestamp: null }],
  };
}

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
