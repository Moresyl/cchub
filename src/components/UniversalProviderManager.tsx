import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight, Cloud, Pencil, Plus, RefreshCw, Save, Trash2, Upload } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

type LocaleText = (zh: string, en: string, ja?: string) => string;
type AppId = "claude" | "codex" | "gemini" | "grokbuild" | "opencode" | "openclaw" | "hermes";

interface UniversalProvider {
  id: string;
  name: string;
  providerType: string;
  baseUrl: string;
  apiKey: string;
  apps: AppId[];
  configs: Record<string, string>;
  websiteUrl?: string | null;
  notes?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

interface Draft {
  id?: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  apps: AppId[];
  websiteUrl: string;
  notes: string;
}

const APP_OPTIONS: { id: AppId; label: string }[] = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "grokbuild", label: "Grok Build" },
  { id: "opencode", label: "OpenCode" },
  { id: "openclaw", label: "OpenClaw" },
  { id: "hermes", label: "Hermes" },
];

function emptyDraft(): Draft {
  return {
    name: "",
    baseUrl: "",
    apiKey: "",
    model: "",
    apps: ["claude", "codex", "gemini", "grokbuild"],
    websiteUrl: "",
    notes: "",
  };
}

function tomlString(value: string) {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function buildSnapshots(draft: Draft): Record<string, string> {
  const metadata = { category: "universal", websiteUrl: draft.websiteUrl || undefined, universal: true };
  const snapshots: Record<string, string> = {};
  for (const app of draft.apps) {
    if (app === "claude") {
      snapshots[app] = JSON.stringify(
        {
          env: { ANTHROPIC_BASE_URL: draft.baseUrl, ANTHROPIC_AUTH_TOKEN: draft.apiKey, ANTHROPIC_MODEL: draft.model },
          metadata,
        },
        null,
        2,
      );
    } else if (app === "codex") {
      snapshots[app] = JSON.stringify(
        {
          auth: { OPENAI_API_KEY: draft.apiKey },
          config: `model_provider = "custom"\nmodel = "${tomlString(draft.model)}"\n[model_providers.custom]\nname = "custom"\nbase_url = "${tomlString(draft.baseUrl)}"\nwire_api = "responses"\nrequires_openai_auth = true`,
          metadata,
        },
        null,
        2,
      );
    } else if (app === "gemini") {
      snapshots[app] = JSON.stringify(
        {
          env: { GOOGLE_GEMINI_BASE_URL: draft.baseUrl, GEMINI_API_KEY: draft.apiKey, GEMINI_MODEL: draft.model },
          metadata,
        },
        null,
        2,
      );
    } else if (app === "grokbuild") {
      const model = tomlString(draft.model || "grok-4.5");
      const config = `[models]\ndefault = "${model}"\n\n[model."${model}"]\nmodel = "${model}"\nbase_url = "${tomlString(draft.baseUrl)}"\nname = "Grok"\napi_backend = "responses"\ncontext_window = 500000\napi_key = "${tomlString(draft.apiKey)}"`;
      snapshots[app] = JSON.stringify({ config, metadata }, null, 2);
    } else if (app === "opencode") {
      snapshots[app] = JSON.stringify(
        {
          npm: "@ai-sdk/openai-compatible",
          name: "custom",
          options: { baseURL: draft.baseUrl, apiKey: draft.apiKey },
          models: { [draft.model || "default"]: { name: draft.model || "default" } },
          metadata,
        },
        null,
        2,
      );
    } else if (app === "openclaw") {
      snapshots[app] = JSON.stringify(
        {
          baseUrl: draft.baseUrl,
          apiKey: draft.apiKey,
          api: "openai-completions",
          models: [{ id: draft.model, name: draft.model }],
          metadata,
        },
        null,
        2,
      );
    } else {
      snapshots[app] = JSON.stringify(
        {
          config: { model: { provider: "custom", default: draft.model, base_url: draft.baseUrl } },
          env: { CCHUB_API_KEY: draft.apiKey },
          metadata: { ...metadata, hermesApiKeyEnv: "CCHUB_API_KEY" },
        },
        null,
        2,
      );
    }
  }
  return snapshots;
}

function draftFromProvider(provider: UniversalProvider): Draft {
  let model = "";
  try {
    const first = JSON.parse(provider.configs[provider.apps[0]] || "{}");
    model =
      first.env?.ANTHROPIC_MODEL ||
      first.env?.GEMINI_MODEL ||
      first.models?.[0]?.id ||
      Object.keys(first.models || {})[0] ||
      first.config?.model?.default ||
      "";
  } catch {
    /* keep empty model */
  }
  return {
    id: provider.id,
    name: provider.name,
    baseUrl: provider.baseUrl,
    apiKey: provider.apiKey,
    model,
    apps: provider.apps,
    websiteUrl: provider.websiteUrl || "",
    notes: provider.notes || "",
  };
}

interface Props {
  locale: string;
  localeText: LocaleText;
  onProfilesChanged: () => void;
}

export default function UniversalProviderManager({ localeText, onProfilesChanged }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [providers, setProviders] = useState<UniversalProvider[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  const load = useCallback(async () => {
    try {
      setProviders(await invoke<UniversalProvider[]>("get_universal_providers"));
    } catch (error) {
      setMessage(String(error));
    }
  }, []);
  useEffect(() => {
    if (expanded) void load();
  }, [expanded, load]);
  const update = <K extends keyof Draft>(key: K, value: Draft[K]) =>
    setDraft((current) => (current ? { ...current, [key]: value } : current));
  const toggleApp = (app: AppId) =>
    setDraft((current) => {
      if (!current) return current;
      const apps = current.apps.includes(app) ? current.apps.filter((item) => item !== app) : [...current.apps, app];
      return { ...current, apps };
    });

  const save = async () => {
    if (!draft) return;
    setBusy(true);
    setMessage("");
    try {
      const saved = await invoke<UniversalProvider>("upsert_universal_provider", {
        provider: {
          id: draft.id || "",
          name: draft.name,
          providerType: "custom",
          baseUrl: draft.baseUrl,
          apiKey: draft.apiKey,
          apps: draft.apps,
          configs: buildSnapshots(draft),
          websiteUrl: draft.websiteUrl || null,
          notes: draft.notes || null,
        },
      });
      setProviders((items) => [saved, ...items.filter((item) => item.id !== saved.id)]);
      setDraft(null);
      setMessage(localeText("已保存统一供应商", "Universal provider saved", "統合 Provider を保存しました"));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };
  const sync = async (provider: UniversalProvider) => {
    setBusy(true);
    setMessage("");
    try {
      await invoke("sync_universal_provider", { id: provider.id });
      onProfilesChanged();
      setMessage(localeText("已同步到所选 App", "Synced to selected apps", "選択した App に同期しました"));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };
  const remove = async (provider: UniversalProvider) => {
    if (
      !window.confirm(
        localeText(`删除「${provider.name}」？`, `Delete “${provider.name}”?`, `「${provider.name}」を削除しますか？`),
      )
    )
      return;
    setBusy(true);
    try {
      await invoke("delete_universal_provider", { id: provider.id });
      setProviders((items) => items.filter((item) => item.id !== provider.id));
      if (draft?.id === provider.id) setDraft(null);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };
  const appLabel = useMemo(() => new Map(APP_OPTIONS.map((item) => [item.id, item.label])), []);

  return (
    <section className="card" style={{ marginBottom: 16 }}>
      <button
        className="btn btn-ghost"
        onClick={() => setExpanded((value) => !value)}
        style={{ width: "100%", justifyContent: "space-between", padding: 0 }}
      >
        <span style={{ display: "flex", alignItems: "center", gap: 8, fontWeight: 600 }}>
          <Cloud size={16} />
          {localeText("统一供应商库", "Universal Providers", "統合 Provider ライブラリ")}
        </span>
        {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
      </button>
      {expanded && (
        <div style={{ marginTop: 14 }}>
          <p className="page-subtitle" style={{ marginBottom: 12 }}>
            {localeText(
              "一次维护 Base URL、密钥和模型，再同步到多个 App。",
              "Maintain one endpoint, key, and model, then sync it to multiple apps.",
              "エンドポイント、キー、モデルを一度管理して複数 App に同期します。",
            )}
          </p>
          <div style={{ display: "flex", gap: 8, marginBottom: 12, flexWrap: "wrap" }}>
            <button className="btn btn-secondary btn-sm" onClick={() => void load()} disabled={busy}>
              <RefreshCw size={14} />
              {localeText("刷新", "Refresh", "更新")}
            </button>
            <button className="btn btn-primary btn-sm" onClick={() => setDraft(emptyDraft())} disabled={busy}>
              <Plus size={14} />
              {localeText("新增供应商", "New provider", "新規 Provider")}
            </button>
          </div>
          {draft && (
            <div
              style={{
                border: "1px solid var(--border)",
                borderRadius: 8,
                padding: 12,
                marginBottom: 12,
                display: "grid",
                gap: 10,
              }}
            >
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
                <input
                  className="input"
                  value={draft.name}
                  onChange={(event) => update("name", event.target.value)}
                  placeholder={localeText("供应商名称", "Provider name", "Provider 名")}
                />
                <input
                  className="input"
                  value={draft.model}
                  onChange={(event) => update("model", event.target.value)}
                  placeholder={localeText("默认模型", "Default model", "既定モデル")}
                />
              </div>
              <input
                className="input"
                value={draft.baseUrl}
                onChange={(event) => update("baseUrl", event.target.value)}
                placeholder="https://api.example.com/v1"
              />
              <input
                className="input"
                type="password"
                value={draft.apiKey}
                onChange={(event) => update("apiKey", event.target.value)}
                placeholder={localeText("API Key", "API key", "API キー")}
              />
              <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
                {APP_OPTIONS.map((app) => (
                  <label key={app.id} style={{ display: "flex", gap: 5, alignItems: "center", fontSize: 12 }}>
                    <input type="checkbox" checked={draft.apps.includes(app.id)} onChange={() => toggleApp(app.id)} />
                    {app.label}
                  </label>
                ))}
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                <button className="btn btn-primary btn-sm" onClick={() => void save()} disabled={busy}>
                  <Save size={14} />
                  {localeText("保存", "Save", "保存")}
                </button>
                <button className="btn btn-ghost btn-sm" onClick={() => setDraft(null)}>
                  {localeText("取消", "Cancel", "キャンセル")}
                </button>
              </div>
            </div>
          )}
          {providers.length === 0 && (
            <div style={{ color: "var(--text-muted)", fontSize: 12 }}>
              {localeText("还没有统一供应商。", "No universal providers yet.", "統合 Provider はまだありません。")}
            </div>
          )}
          <div style={{ display: "grid", gap: 8 }}>
            {providers.map((provider) => (
              <div
                key={provider.id}
                style={{
                  border: "1px solid var(--border)",
                  borderRadius: 8,
                  padding: 10,
                  display: "flex",
                  justifyContent: "space-between",
                  gap: 12,
                  alignItems: "center",
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 600 }}>{provider.name}</div>
                  <div
                    style={{
                      color: "var(--text-muted)",
                      fontSize: 12,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {provider.baseUrl} · {provider.apps.map((app) => appLabel.get(app)).join(", ")}
                  </div>
                </div>
                <div style={{ display: "flex", gap: 5, flexShrink: 0 }}>
                  <button
                    className="btn btn-secondary btn-icon-sm"
                    title={localeText("编辑", "Edit", "編集")}
                    onClick={() => setDraft(draftFromProvider(provider))}
                  >
                    <Pencil size={14} />
                  </button>
                  <button className="btn btn-secondary btn-sm" onClick={() => void sync(provider)} disabled={busy}>
                    <Upload size={14} />
                    {localeText("同步", "Sync", "同期")}
                  </button>
                  <button
                    className="btn btn-ghost btn-icon-sm"
                    title={localeText("删除", "Delete", "削除")}
                    onClick={() => void remove(provider)}
                    disabled={busy}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
          {message && <div style={{ marginTop: 10, color: "var(--text-muted)", fontSize: 12 }}>{message}</div>}
        </div>
      )}
    </section>
  );
}
