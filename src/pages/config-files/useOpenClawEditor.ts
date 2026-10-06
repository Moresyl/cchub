import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type NativeDocument = Record<string, unknown>;
export function object(value: unknown): value is NativeDocument {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
export function hasOwn(value: object, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}
export function at(root: NativeDocument, path: readonly string[]): unknown {
  let value: unknown = root;
  for (const key of path) {
    if ((!object(value) && !Array.isArray(value)) || !hasOwn(value, key)) return undefined;
    value = (value as NativeDocument)[key];
  }
  return value;
}
export function patchDocument(root: NativeDocument, path: readonly string[], value: unknown): NativeDocument {
  if (!path.length) throw new Error("A field path is required");
  const next = structuredClone(root);
  let parent: NativeDocument | unknown[] = next;
  for (const key of path.slice(0, -1)) {
    if (Array.isArray(parent) && (!/^(0|[1-9]\d*)$/.test(key) || Number(key) >= parent.length))
      throw new Error("Invalid model index");
    const child: unknown = hasOwn(parent, key) ? (parent as NativeDocument)[key] : undefined;
    if (child !== undefined && !object(child) && !Array.isArray(child))
      throw new Error("Correct the containing object in the raw configuration first");
    if (!object(child) && !Array.isArray(child))
      Object.defineProperty(parent, key, { value: {}, enumerable: true, configurable: true, writable: true });
    parent = (parent as NativeDocument)[key] as NativeDocument | unknown[];
  }
  const key = path[path.length - 1];
  if (Array.isArray(parent) && (!/^(0|[1-9]\d*)$/.test(key) || Number(key) >= parent.length || value === undefined))
    throw new Error("Invalid array field edit");
  if (value === undefined) delete (parent as NativeDocument)[key];
  else Object.defineProperty(parent, key, { value, enumerable: true, configurable: true, writable: true });
  return next;
}
export function isOpenClawConfigFile(root: string, file: string | null, configured: string | undefined) {
  const normalize = (path: string) => {
    const value = path.replace(/\\/g, "/");
    return /^[a-z]:|^\/\//i.test(value) ? value.toLowerCase() : value;
  };
  return root === "openclaw" && !!file && !!configured && normalize(file) === normalize(configured);
}

export function useOpenClawEditor(content: string, scope: string, active: boolean) {
  const [parsed, setParsed] = useState<{ content: string; scope: string; data: NativeDocument } | null>(null);
  const [draft, setDraft] = useState<(NonNullable<typeof parsed> & { numbers: string[][] }) | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<{ content: string; scope: string } | null>(null);
  const generation = useRef(0);
  const preparing = useRef(false);
  useEffect(() => {
    const requests = generation;
    const request = ++generation.current;
    if (!active) return;
    void invoke<unknown>("parse_openclaw_config_content", { content })
      .then((data) => {
        if (request !== generation.current) return;
        if (!object(data)) throw new Error("Invalid configuration root");
        setParsed({ content, scope, data });
        setFailure(null);
      })
      .catch(() => {
        if (request === generation.current) setFailure({ content, scope });
      });
    return () => {
      requests.current++;
    };
  }, [content, scope, active]);
  const current = parsed?.content === content && parsed.scope === scope ? parsed : null;
  const pending = draft?.content === content && draft.scope === scope ? draft : null;
  const failed = failure?.content === content && failure.scope === scope;
  const data = pending?.data ?? current?.data ?? null;
  const dirty = active && !!pending && JSON.stringify(pending.data) !== JSON.stringify(current?.data);
  const invalidNumber =
    pending?.numbers.some((path) => {
      const value = at(pending.data, path);
      if (value === undefined || (typeof value === "string" && !value.trim())) return false;
      const number = Number(value);
      return (
        !Number.isFinite(number) ||
        number < 0 ||
        (path[path.length - 1] === "contextWindow" && (!Number.isSafeInteger(number) || number <= 0))
      );
    }) ?? false;
  return {
    data,
    loading: active && !current && !failed,
    error: active && !!failed,
    hasPendingDraft: dirty,
    busy,
    invalidNumber,
    numbers: pending?.numbers ?? [],
    update: (next: NativeDocument, numbers = pending?.numbers ?? []) => {
      if (current && !preparing.current) setDraft({ content, scope, data: next, numbers });
    },
    updateNumber: (path: string[], value: string) => {
      if (current && data && !preparing.current)
        setDraft({
          content,
          scope,
          data: patchDocument(data, path, value),
          numbers: [
            ...(pending?.numbers ?? []).filter((entry) => JSON.stringify(entry) !== JSON.stringify(path)),
            path,
          ],
        });
    },
    reset: () => {
      if (!preparing.current) setDraft(null);
    },
    prepare: async () => {
      if (!active) return content;
      if (!current || failed || preparing.current) throw new Error("Correct the raw JSON5 configuration before saving");
      if (!dirty) return content;
      if (invalidNumber)
        throw new Error("Enter valid nonnegative costs and a positive integer context window before saving");
      preparing.current = true;
      setBusy(true);
      try {
        let desired = pending!.data;
        for (const path of pending!.numbers) {
          const value = at(desired, path);
          desired = patchDocument(
            desired,
            path,
            value === undefined || (typeof value === "string" && !value.trim()) ? undefined : Number(value),
          );
        }
        return await invoke<string>("edit_openclaw_config_content", { content, desired });
      } finally {
        preparing.current = false;
        setBusy(false);
      }
    },
  };
}
export type OpenClawEditor = ReturnType<typeof useOpenClawEditor>;
