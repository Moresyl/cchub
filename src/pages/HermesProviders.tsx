import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Loader2,
  Plus,
  Save,
  Trash2,
  X,
  Zap,
} from "lucide-react";
import { t, tReplace } from "../lib/i18n";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";

interface HermesProvider {
  name: string;
  baseUrl: string;
  apiMode: string;
  model: string;
  apiKeyEnv: string;
}

const API_MODES = [
  { value: "chat_completions", label: "Chat Completions (OpenAI)" },
  { value: "anthropic_messages", label: "Anthropic Messages" },
  { value: "codex_responses", label: "Codex Responses" },
  { value: "bedrock_converse", label: "Bedrock Converse" },
];

const KNOWN_PRESETS: Record<string, Partial<HermesProvider>> = {
  openrouter: { baseUrl: "https://openrouter.ai/api/v1", apiKeyEnv: "OPENROUTER_API_KEY", apiMode: "chat_completions" },
  anthropic: { baseUrl: "https://api.anthropic.com", apiKeyEnv: "ANTHROPIC_API_KEY", apiMode: "anthropic_messages" },
  gemini: { baseUrl: "https://generativelanguage.googleapis.com/v1beta", apiKeyEnv: "GEMINI_API_KEY", apiMode: "chat_completions" },
  nous: { baseUrl: "https://portal.nousresearch.com/v1", apiKeyEnv: "", apiMode: "chat_completions" },
  zai: { baseUrl: "https://api.z.ai/v1", apiKeyEnv: "GLM_API_KEY", apiMode: "chat_completions" },
  "kimi-coding": { baseUrl: "https://api.moonshot.ai/v1", apiKeyEnv: "KIMI_API_KEY", apiMode: "chat_completions" },
};

function emptyProvider(): HermesProvider {
  return { name: "", baseUrl: "", apiMode: "chat_completions", model: "", apiKeyEnv: "" };
}

interface HermesProvidersProps {
  embedded?: boolean;
}

export default function HermesProviders({ embedded = false }: HermesProvidersProps = {}) {
  const i = t();
  const [providers, setProviders] = useState<HermesProvider[]>([]);
  const [activeProvider, setActiveProvider] = useState("");
  const [loading, setLoading] = useState(true);
  const [notInstalled, setNotInstalled] = useState(false);
  const [editing, setEditing] = useState<HermesProvider | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [saving, setSaving] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<HermesProvider | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [list, active] = await Promise.all([
        invoke<HermesProvider[]>("list_hermes_providers"),
        invoke<string>("get_hermes_active_provider"),
      ]);
      setProviders(list);
      setActiveProvider(active);
      setNotInstalled(false);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("Cannot find") || msg.includes("not found") || msg.includes("No such file")) {
        setNotInstalled(true);
      } else {
        showToast("error", msg);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void load(); }, [load]);

  const handleAdd = useCallback(() => {
    setEditing(emptyProvider());
    setIsNew(true);
  }, []);

  const handleEdit = useCallback((p: HermesProvider) => {
    setEditing({ ...p });
    setIsNew(false);
  }, []);

  const handleSave = useCallback(async () => {
    if (!editing || !editing.name.trim()) return;
    setSaving(true);
    try {
      await invoke("save_hermes_provider", { provider: editing });
      showToast("success", i.hermesProviders.saveSuccess);
      setEditing(null);
      await load();
    } catch (e) {
      showToast("error", `${i.hermesProviders.saveFailed}: ${e}`);
    } finally {
      setSaving(false);
    }
  }, [editing, i.hermesProviders, load]);

  const doDelete = useCallback(async (p: HermesProvider) => {
    try {
      await invoke("delete_hermes_provider", { name: p.name });
      showToast("success", i.hermesProviders.deleteSuccess);
      if (editing?.name === p.name) setEditing(null);
      await load();
    } catch (e) {
      showToast("error", `${i.hermesProviders.deleteFailed}: ${e}`);
    }
  }, [editing, i.hermesProviders, load]);

  const handleSetActive = useCallback(async (name: string) => {
    try {
      await invoke("set_hermes_active_provider", { name });
      showToast("success", i.hermesProviders.setActiveSuccess);
      setActiveProvider(name);
    } catch (e) {
      showToast("error", `${i.hermesProviders.setActiveFailed}: ${e}`);
    }
  }, [i.hermesProviders]);

  const applyPreset = useCallback((presetName: string) => {
    const preset = KNOWN_PRESETS[presetName];
    if (preset && editing) {
      setEditing({
        ...editing,
        name: editing.name || presetName,
        baseUrl: preset.baseUrl || editing.baseUrl,
        apiMode: preset.apiMode || editing.apiMode,
        apiKeyEnv: preset.apiKeyEnv ?? editing.apiKeyEnv,
      });
    }
  }, [editing]);

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
      </div>
    );
  }

  if (notInstalled) {
    return (
      <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
        {!embedded && (
          <div className="page-header">
            <div>
              <h2 className="page-title">{i.hermesProviders.title}</h2>
              <p className="page-subtitle">{i.hermesProviders.subtitle}</p>
            </div>
          </div>
        )}
        <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
          <div className="card" style={{ padding: 32, textAlign: "center", maxWidth: 400 }}>
            <h3 style={{ fontSize: 15, marginBottom: 8 }}>{i.hermesProviders.notInstalled}</h3>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.hermesProviders.notInstalledDesc}</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {!embedded && (
        <div className="page-header">
          <div>
            <h2 className="page-title">{i.hermesProviders.title}</h2>
            <p className="page-subtitle">{i.hermesProviders.subtitle}</p>
          </div>
          <button className="btn btn-primary btn-sm" onClick={handleAdd} style={{ display: "flex", alignItems: "center", gap: 5 }}>
            <Plus size={14} />
            {i.hermesProviders.addProvider}
          </button>
        </div>
      )}
      {embedded && (
        <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 12 }}>
          <button className="btn btn-primary btn-sm" onClick={handleAdd} style={{ display: "flex", alignItems: "center", gap: 5 }}>
            <Plus size={14} />
            {i.hermesProviders.addProvider}
          </button>
        </div>
      )}

      <div style={{ flex: 1, overflow: "auto", padding: embedded ? 0 : "16px 20px" }}>
        {providers.length === 0 && !editing ? (
          <div className="card" style={{ padding: 32, textAlign: "center" }}>
            <h3 style={{ fontSize: 14, marginBottom: 8 }}>{i.hermesProviders.noProviders}</h3>
            <p style={{ fontSize: 12, color: "var(--text-muted)" }}>{i.hermesProviders.noProvidersDesc}</p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {providers.map((p) => (
              <div
                key={p.name}
                className="card"
                style={{
                  padding: "12px 16px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  cursor: "pointer",
                  border: activeProvider === p.name ? "1px solid var(--accent)" : undefined,
                }}
                onClick={() => handleEdit(p)}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span style={{ fontSize: 13, fontWeight: 600 }}>{p.name}</span>
                    {activeProvider === p.name && (
                      <span style={{
                        fontSize: 10,
                        padding: "1px 6px",
                        borderRadius: 8,
                        background: "var(--accent)",
                        color: "#fff",
                        fontWeight: 500,
                      }}>
                        {i.hermesProviders.activeProvider}
                      </span>
                    )}
                  </div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 3 }}>
                    {p.apiMode} · {p.baseUrl || "—"} · {p.model || "—"}
                  </div>
                </div>
                <div style={{ display: "flex", gap: 4 }} onClick={(e) => e.stopPropagation()}>
                  {activeProvider !== p.name && (
                    <button
                      className="btn btn-ghost btn-icon-sm"
                      title={i.hermesProviders.setActive}
                      onClick={() => void handleSetActive(p.name)}
                    >
                      <Zap size={13} />
                    </button>
                  )}
                  <button
                    className="btn btn-ghost btn-icon-sm"
                    title={i.hermesProviders.deleteProvider}
                    onClick={() => setPendingDelete(p)}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Edit/Create Panel */}
        {editing && (
          <div className="card" style={{ marginTop: 16, padding: "16px 20px" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 14 }}>
              <h4 style={{ fontSize: 13, fontWeight: 700 }}>
                {isNew ? i.hermesProviders.addProvider : i.hermesProviders.editProvider}
              </h4>
              <button className="btn btn-ghost btn-icon-sm" onClick={() => setEditing(null)}>
                <X size={14} />
              </button>
            </div>

            {/* Presets quick-fill (only for new) */}
            {isNew && (
              <div style={{ marginBottom: 12 }}>
                <div style={{ fontSize: 11, color: "var(--text-secondary)", marginBottom: 6 }}>Presets</div>
                <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                  {Object.keys(KNOWN_PRESETS).map((key) => (
                    <button
                      key={key}
                      className="btn btn-secondary btn-xs"
                      onClick={() => applyPreset(key)}
                    >
                      {key}
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <label className="field-label">{i.hermesProviders.name}</label>
                <input
                  className="input input-sm"
                  value={editing.name}
                  onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                  placeholder={i.hermesProviders.namePlaceholder}
                  disabled={!isNew}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <label className="field-label">{i.hermesProviders.apiMode}</label>
                <select
                  className="input input-sm"
                  value={editing.apiMode}
                  onChange={(e) => setEditing({ ...editing, apiMode: e.target.value })}
                >
                  {API_MODES.map((m) => (
                    <option key={m.value} value={m.value}>{m.label}</option>
                  ))}
                </select>
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4, gridColumn: "1 / -1" }}>
                <label className="field-label">{i.hermesProviders.baseUrl}</label>
                <input
                  className="input input-sm"
                  value={editing.baseUrl}
                  onChange={(e) => setEditing({ ...editing, baseUrl: e.target.value })}
                  placeholder={i.hermesProviders.baseUrlPlaceholder}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <label className="field-label">{i.hermesProviders.model}</label>
                <input
                  className="input input-sm"
                  value={editing.model}
                  onChange={(e) => setEditing({ ...editing, model: e.target.value })}
                  placeholder={i.hermesProviders.modelPlaceholder}
                />
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                <label className="field-label">{i.hermesProviders.apiKeyEnv}</label>
                <input
                  className="input input-sm"
                  value={editing.apiKeyEnv}
                  onChange={(e) => setEditing({ ...editing, apiKeyEnv: e.target.value })}
                  placeholder={i.hermesProviders.apiKeyEnvPlaceholder}
                />
              </div>
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 14, gap: 8 }}>
              <button className="btn btn-secondary btn-sm" onClick={() => setEditing(null)}>
                <X size={12} />
              </button>
              <button
                className="btn btn-primary btn-sm"
                onClick={() => void handleSave()}
                disabled={saving || !editing.name.trim()}
                style={{ display: "flex", alignItems: "center", gap: 5 }}
              >
                {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
                {i.hermesProviders.save}
              </button>
            </div>
          </div>
        )}
      </div>

      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={i.hermesProviders.deleteProvider}
        message={pendingDelete ? tReplace(i.hermesProviders.confirmDelete, { name: pendingDelete.name }) : ""}
        confirmText={i.hermesProviders.deleteProvider}
        variant="destructive"
        onConfirm={() => { if (pendingDelete) void doDelete(pendingDelete); setPendingDelete(null); }}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
}
