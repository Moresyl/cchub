export interface ProxyModelAlias {
  model: string;
  upstream: string;
}

export interface AliasValidationError {
  kind: "shape" | "limit" | "invalid" | "duplicate" | "expanded";
  row?: number;
}

const bytes = (value: string) => new TextEncoder().encode(value).length;
const validName = (value: string, template: boolean) =>
  !!value && bytes(value) <= 1024 && (template ? /^[\p{L}\p{N}._\-/:@+*]+$/u : /^[\p{L}\p{N}._\-/:@+]+$/u).test(value);

export function parseModelAliases(value: unknown): {
  localProxyModelAliases?: ProxyModelAlias[];
  localProxyModelAliasesRaw?: unknown;
} {
  if (value === undefined) return {};
  if (
    Array.isArray(value) &&
    value.every((row) => row && typeof row.model === "string" && typeof row.upstream === "string")
  ) {
    return { localProxyModelAliases: value.map(({ model, upstream }) => ({ model, upstream })) };
  }
  return { localProxyModelAliasesRaw: value };
}

/** Incomplete rows remain in the snapshot so saving cannot silently discard them. */
export function serializeModelAliases(rows: ProxyModelAlias[] | undefined, raw?: unknown): unknown {
  if (raw !== undefined) return raw;
  if (rows === undefined) return undefined;
  return rows
    .map(({ model, upstream }) => ({ model: model.trim(), upstream: upstream.trim() }))
    .filter(({ model, upstream }) => model || upstream);
}

export function validateModelAliases(rows: ProxyModelAlias[], raw?: unknown): AliasValidationError | null {
  if (raw !== undefined) return { kind: "shape" };
  if (rows.length > 128) return { kind: "limit" };
  const seen = new Set<string>();
  for (const [index, row] of rows.entries()) {
    const model = row.model.trim();
    const upstream = row.upstream.trim();
    if (!model && !upstream) continue;
    if (!(model === "*" || validName(model, false)) || !validName(upstream, true)) {
      return { kind: "invalid", row: index + 1 };
    }
    if (seen.has(model)) return { kind: "duplicate", row: index + 1 };
    seen.add(model);
  }
  return null;
}

export function previewModelAlias(rows: ProxyModelAlias[], model: string): string | null {
  if (validateModelAliases(rows)) return null;
  const clean = rows.map((row) => ({ model: row.model.trim(), upstream: row.upstream.trim() }));
  const alias = clean.find((row) => row.model === model) ?? clean.find((row) => row.model === "*");
  if (!alias) return model;
  const stars = alias.upstream.split("*").length - 1;
  if (bytes(alias.upstream) - stars + stars * bytes(model) > 1024) return null;
  const result = alias.upstream.split("*").join(model);
  return validName(result, false) ? result : null;
}
