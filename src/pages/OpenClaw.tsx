import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  Bot,
  Loader2,
  Plus,
  Save,
  Settings2,
  Shield,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";

type Tab = "env" | "tools" | "agents";

interface OpenClawStatus {
  installed: boolean;
  configPath: string;
}

interface HealthWarning {
  code: string;
  message: string;
  path: string | null;
}

interface EnvConfig {
  [key: string]: unknown;
}

interface ToolsConfig {
  profile: string | null;
  allow: string[];
  deny: string[];
}

interface AgentsDefaults {
  model: { primary: string; fallbacks: string[] } | null;
  models: Record<string, { alias: string | null }> | null;
}

const TOOL_PROFILES = ["minimal", "coding", "messaging", "full"];

function OpenClaw() {
  const i = t();
  const [tab, setTab] = useState<Tab>("env");
  const [status, setStatus] = useState<OpenClawStatus | null>(null);
  const [warnings, setWarnings] = useState<HealthWarning[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void loadStatus();
  }, []);

  async function loadStatus() {
    try {
      const [s, w] = await Promise.all([
        invoke<OpenClawStatus>("get_openclaw_status"),
        invoke<HealthWarning[]>("scan_openclaw_health"),
      ]);
      setStatus(s);
      setWarnings(w);
    } catch {
      setStatus({ installed: false, configPath: "" });
    } finally {
      setLoading(false);
    }
  }

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {i.openClaw.loading}
        </span>
      </div>
    );
  }

  if (!status?.installed) {
    return (
      <div style={{ height: "100%", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 12 }}>
        <Bot size={40} style={{ color: "var(--text-muted)" }} />
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
          {i.openClaw.notInstalled}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-secondary)", maxWidth: 400, textAlign: "center" }}>
          {i.openClaw.notInstalledDesc}
        </div>
      </div>
    );
  }

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", gap: 0 }}>
      {/* Header */}
      <div className="page-header">
        <div>
          <h2 className="page-title">{i.openClaw.title}</h2>
          <p className="page-subtitle">{i.openClaw.subtitle}</p>
        </div>
      </div>

      {/* Health warnings */}
      {warnings.length > 0 && (
        <div style={{ padding: "0 20px 12px" }}>
          {warnings.map((w, idx) => (
            <div key={idx} style={{ display: "flex", alignItems: "center", gap: 8, padding: "6px 10px", borderRadius: 6, background: "var(--warning-bg, rgba(234, 179, 8, 0.08))", border: "1px solid var(--warning, #eab308)", marginBottom: 6, fontSize: 12 }}>
              <AlertTriangle size={13} style={{ color: "var(--warning)", flexShrink: 0 }} />
              <span style={{ color: "var(--text-secondary)" }}>{w.message}</span>
              {w.path && <code style={{ fontSize: 10, color: "var(--text-muted)", marginLeft: "auto" }}>{w.path}</code>}
            </div>
          ))}
        </div>
      )}

      {/* Tabs */}
      <div style={{ display: "flex", gap: 4, padding: "0 20px 12px", borderBottom: "1px solid var(--border-default)" }}>
        {([
          { key: "env" as Tab, icon: Settings2, label: i.openClaw.envTab },
          { key: "tools" as Tab, icon: Wrench, label: i.openClaw.toolsTab },
          { key: "agents" as Tab, icon: Shield, label: i.openClaw.agentsTab },
        ]).map(({ key, icon: Icon, label }) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            className={`btn btn-sm ${tab === key ? "btn-primary" : "btn-ghost"}`}
            style={{ display: "flex", alignItems: "center", gap: 5 }}
          >
            <Icon size={13} />
            {label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div style={{ flex: 1, overflow: "auto", padding: "16px 20px" }}>
        {tab === "env" && <EnvPanel />}
        {tab === "tools" && <ToolsPanel />}
        {tab === "agents" && <AgentsPanel />}
      </div>
    </div>
  );
}

// ── Env Panel ──

function EnvPanel() {
  const i = t();
  const [vars, setVars] = useState<Array<{ key: string; value: string }>>([]);
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void loadEnv();
  }, []);

  async function loadEnv() {
    try {
      const data = await invoke<EnvConfig>("get_openclaw_env");
      const entries = Object.entries(data).map(([key, value]) => ({
        key,
        value: typeof value === "string" ? value : JSON.stringify(value),
      }));
      setVars(entries);
    } catch {
      setVars([]);
    } finally {
      setLoaded(true);
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      const envObj: Record<string, string> = {};
      for (const { key, value } of vars) {
        if (key.trim()) envObj[key.trim()] = value;
      }
      await invoke("set_openclaw_env", { env: envObj });
      showToast("success", i.openClaw.saveSuccess);
    } catch (e) {
      showToast("error", `${i.openClaw.saveFailed}: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  function addVar() {
    setVars([...vars, { key: "", value: "" }]);
  }

  function removeVar(idx: number) {
    setVars(vars.filter((_, i) => i !== idx));
  }

  function updateVar(idx: number, field: "key" | "value", val: string) {
    setVars(vars.map((v, i) => i === idx ? { ...v, [field]: val } : v));
  }

  if (!loaded) return <LoadingSpinner />;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 4 }}>
        {i.openClaw.envDesc}
      </div>

      {vars.map((v, idx) => (
        <div key={idx} style={{ display: "flex", gap: 8, alignItems: "center" }}>
          <input
            className="input input-sm"
            style={{ flex: 1 }}
            placeholder="KEY"
            value={v.key}
            onChange={(e) => updateVar(idx, "key", e.target.value)}
          />
          <input
            className="input input-sm"
            style={{ flex: 2 }}
            placeholder="value"
            value={v.value}
            onChange={(e) => updateVar(idx, "value", e.target.value)}
          />
          <button className="btn btn-ghost btn-icon-sm" onClick={() => removeVar(idx)}>
            <Trash2 size={13} />
          </button>
        </div>
      ))}

      <div style={{ display: "flex", gap: 8 }}>
        <button className="btn btn-secondary btn-sm" onClick={addVar} style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <Plus size={13} />
          {i.openClaw.addVar}
        </button>
        <button
          className="btn btn-primary btn-sm"
          onClick={() => void handleSave()}
          disabled={saving}
          style={{ display: "flex", alignItems: "center", gap: 5, marginLeft: "auto" }}
        >
          {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
          {i.openClaw.save}
        </button>
      </div>
    </div>
  );
}

// ── Tools Panel ──

function ToolsPanel() {
  const i = t();
  const [config, setConfig] = useState<ToolsConfig>({ profile: null, allow: [], deny: [] });
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);
  const [newAllow, setNewAllow] = useState("");
  const [newDeny, setNewDeny] = useState("");

  useEffect(() => {
    void loadTools();
  }, []);

  async function loadTools() {
    try {
      const data = await invoke<ToolsConfig>("get_openclaw_tools");
      // Backend uses skip_serializing_if = "Vec::is_empty", so allow/deny may
      // be undefined when empty. Normalize to empty arrays.
      setConfig({
        profile: data?.profile ?? null,
        allow: data?.allow ?? [],
        deny: data?.deny ?? [],
      });
    } catch {
      // keep defaults
    } finally {
      setLoaded(true);
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      await invoke("set_openclaw_tools", { tools: config });
      showToast("success", i.openClaw.saveSuccess);
    } catch (e) {
      showToast("error", `${i.openClaw.saveFailed}: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  if (!loaded) return <LoadingSpinner />;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {/* Profile select */}
      <div>
        <label style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", display: "block", marginBottom: 6 }}>
          {i.openClaw.toolProfile}
        </label>
        <select
          className="input input-sm"
          style={{ width: 200 }}
          value={config.profile ?? ""}
          onChange={(e) => setConfig({ ...config, profile: e.target.value || null })}
        >
          <option value="">{i.openClaw.noProfile}</option>
          {TOOL_PROFILES.map((p) => (
            <option key={p} value={p}>{p}</option>
          ))}
        </select>
      </div>

      {/* Allow list */}
      <div>
        <label style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", display: "block", marginBottom: 6 }}>
          {i.openClaw.allowList}
        </label>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
          {config.allow.map((item, idx) => (
            <span key={idx} style={{ display: "inline-flex", alignItems: "center", gap: 4, padding: "2px 8px", borderRadius: 12, background: "var(--bg-app)", border: "1px solid var(--border-default)", fontSize: 11 }}>
              {item}
              <button style={{ border: "none", background: "none", cursor: "pointer", padding: 0, color: "var(--text-muted)" }} onClick={() => setConfig({ ...config, allow: config.allow.filter((_, i) => i !== idx) })}>
                <X size={10} />
              </button>
            </span>
          ))}
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          <input
            className="input input-sm"
            style={{ width: 200 }}
            placeholder={i.openClaw.toolName}
            value={newAllow}
            onChange={(e) => setNewAllow(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newAllow.trim()) {
                setConfig({ ...config, allow: [...config.allow, newAllow.trim()] });
                setNewAllow("");
              }
            }}
          />
          <button className="btn btn-secondary btn-sm" onClick={() => {
            if (newAllow.trim()) {
              setConfig({ ...config, allow: [...config.allow, newAllow.trim()] });
              setNewAllow("");
            }
          }}>
            <Plus size={13} />
          </button>
        </div>
      </div>

      {/* Deny list */}
      <div>
        <label style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", display: "block", marginBottom: 6 }}>
          {i.openClaw.denyList}
        </label>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
          {config.deny.map((item, idx) => (
            <span key={idx} style={{ display: "inline-flex", alignItems: "center", gap: 4, padding: "2px 8px", borderRadius: 12, background: "var(--bg-app)", border: "1px solid var(--error, #ef4444)", fontSize: 11 }}>
              {item}
              <button style={{ border: "none", background: "none", cursor: "pointer", padding: 0, color: "var(--text-muted)" }} onClick={() => setConfig({ ...config, deny: config.deny.filter((_, i) => i !== idx) })}>
                <X size={10} />
              </button>
            </span>
          ))}
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          <input
            className="input input-sm"
            style={{ width: 200 }}
            placeholder={i.openClaw.toolName}
            value={newDeny}
            onChange={(e) => setNewDeny(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && newDeny.trim()) {
                setConfig({ ...config, deny: [...config.deny, newDeny.trim()] });
                setNewDeny("");
              }
            }}
          />
          <button className="btn btn-secondary btn-sm" onClick={() => {
            if (newDeny.trim()) {
              setConfig({ ...config, deny: [...config.deny, newDeny.trim()] });
              setNewDeny("");
            }
          }}>
            <Plus size={13} />
          </button>
        </div>
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button
          className="btn btn-primary btn-sm"
          onClick={() => void handleSave()}
          disabled={saving}
          style={{ display: "flex", alignItems: "center", gap: 5 }}
        >
          {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
          {i.openClaw.save}
        </button>
      </div>
    </div>
  );
}

// ── Agents Panel ──

function AgentsPanel() {
  const i = t();
  const [defaults, setDefaults] = useState<AgentsDefaults>({ model: null, models: null });
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void loadAgents();
  }, []);

  async function loadAgents() {
    try {
      const data = await invoke<AgentsDefaults>("get_openclaw_agents_defaults");
      // Normalize: model.fallbacks may be undefined due to backend's
      // skip_serializing_if = "Vec::is_empty"; ensure it's at least [].
      setDefaults({
        model: data?.model
          ? { primary: data.model.primary ?? "", fallbacks: data.model.fallbacks ?? [] }
          : null,
        models: data?.models ?? null,
      });
    } catch {
      // keep defaults
    } finally {
      setLoaded(true);
    }
  }

  async function handleSave() {
    setSaving(true);
    try {
      await invoke("set_openclaw_agents_defaults", { defaults });
      showToast("success", i.openClaw.saveSuccess);
    } catch (e) {
      showToast("error", `${i.openClaw.saveFailed}: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  if (!loaded) return <LoadingSpinner />;

  const model = defaults.model ?? { primary: "", fallbacks: [] };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {/* Primary model */}
      <div>
        <label style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", display: "block", marginBottom: 6 }}>
          {i.openClaw.primaryModel}
        </label>
        <input
          className="input input-sm"
          style={{ width: 320 }}
          placeholder="claude-sonnet-4-20250514"
          value={model.primary}
          onChange={(e) => setDefaults({ ...defaults, model: { ...model, primary: e.target.value } })}
        />
      </div>

      {/* Fallbacks */}
      <div>
        <label style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", display: "block", marginBottom: 6 }}>
          {i.openClaw.fallbackModels}
        </label>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {model.fallbacks.map((fb, idx) => (
            <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center" }}>
              <input
                className="input input-sm"
                style={{ width: 280 }}
                value={fb}
                onChange={(e) => {
                  const newFallbacks = [...model.fallbacks];
                  newFallbacks[idx] = e.target.value;
                  setDefaults({ ...defaults, model: { ...model, fallbacks: newFallbacks } });
                }}
              />
              <button className="btn btn-ghost btn-icon-sm" onClick={() => {
                const newFallbacks = model.fallbacks.filter((_, i) => i !== idx);
                setDefaults({ ...defaults, model: { ...model, fallbacks: newFallbacks } });
              }}>
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          <button
            className="btn btn-secondary btn-sm"
            style={{ width: "fit-content", display: "flex", alignItems: "center", gap: 5 }}
            onClick={() => setDefaults({ ...defaults, model: { ...model, fallbacks: [...model.fallbacks, ""] } })}
          >
            <Plus size={13} />
            {i.openClaw.addFallback}
          </button>
        </div>
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button
          className="btn btn-primary btn-sm"
          onClick={() => void handleSave()}
          disabled={saving}
          style={{ display: "flex", alignItems: "center", gap: 5 }}
        >
          {saving ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
          {i.openClaw.save}
        </button>
      </div>
    </div>
  );
}

function LoadingSpinner() {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: 120 }}>
      <div className="spinner" />
    </div>
  );
}

export default OpenClaw;
