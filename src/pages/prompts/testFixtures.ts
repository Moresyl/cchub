import type { LibrarySnapshot, PromptRecord } from "./types";
export function record(overrides: Partial<PromptRecord> = {}): PromptRecord {
  return {
    id: "one",
    name: "Fixture instructions",
    content: "# Fixture content",
    description: "Fixture description",
    enabled: true,
    createdAt: 1000,
    updatedAt: 2000,
    ...overrides,
  };
}
export function snapshot(overrides: Partial<LibrarySnapshot> = {}): LibrarySnapshot {
  const prompt = record();
  return {
    prompts: { [prompt.id]: prompt },
    libraryRevision: "library-1",
    live: { content: prompt.content, revision: "file-1" },
    liveError: null,
    ...overrides,
  };
}
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}
