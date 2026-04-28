import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Brain, Loader2, Save, User } from "lucide-react";
import { t } from "../lib/i18n";
import { showToast } from "../components/Toast";
import MarkdownEditor from "../components/MarkdownEditor";

type MemoryKind = "memory" | "user";

interface MemoryLimits {
  memory: number;
  user: number;
  memoryEnabled: boolean;
  userEnabled: boolean;
}

function HermesMemory() {
  const i = t();
  const [activeTab, setActiveTab] = useState<MemoryKind>("memory");
  const [limits, setLimits] = useState<MemoryLimits | null>(null);
  const [content, setContent] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [saving, setSaving] = useState(false);

  const fetchLimits = useCallback(async () => {
    try {
      const data = await invoke<MemoryLimits>("get_hermes_memory_limits");
      setLimits(data);
    } catch {
      // hermes not installed — silent fail
    }
  }, []);

  const fetchContent = useCallback(async (kind: MemoryKind) => {
    try {
      const data = await invoke<string>("get_hermes_memory_content", { kind });
      setContent(data);
      setLoaded(true);
    } catch {
      setContent("");
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    fetchLimits();
  }, [fetchLimits]);

  useEffect(() => {
    setLoaded(false);
    fetchContent(activeTab);
  }, [activeTab, fetchContent]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await invoke("save_hermes_memory_content", { kind: activeTab, content });
      showToast("success", i.hermesMemory.saveSuccess);
    } catch (e) {
      showToast("error", `${i.hermesMemory.saveFailed}: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const handleToggle = async (enabled: boolean) => {
    try {
      await invoke("toggle_hermes_memory_enabled", { kind: activeTab, enabled });
      await fetchLimits();
      showToast("success", enabled ? i.hermesMemory.enabled : i.hermesMemory.disabled);
    } catch (e) {
      showToast("error", `${i.hermesMemory.operationFailed}: ${e}`);
    }
  };

  const currentLimit = activeTab === "memory" ? (limits?.memory ?? 2200) : (limits?.user ?? 1375);
  const currentEnabled = activeTab === "memory" ? (limits?.memoryEnabled ?? true) : (limits?.userEnabled ?? true);
  const charCount = content.length;
  const isOver = charCount > currentLimit;

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "12px 20px", borderBottom: "1px solid var(--border-default)" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 4, background: "var(--bg-app)", borderRadius: 8, padding: 3 }}>
          <button
            onClick={() => setActiveTab("memory")}
            className={`btn btn-sm ${activeTab === "memory" ? "btn-primary" : "btn-ghost"}`}
            style={{ display: "flex", alignItems: "center", gap: 6 }}
          >
            <Brain size={14} />
            MEMORY.md
          </button>
          <button
            onClick={() => setActiveTab("user")}
            className={`btn btn-sm ${activeTab === "user" ? "btn-primary" : "btn-ghost"}`}
            style={{ display: "flex", alignItems: "center", gap: 6 }}
          >
            <User size={14} />
            USER.md
          </button>
        </div>

        <label style={{ display: "flex", alignItems: "center", gap: 8, cursor: "pointer" }}>
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>
            {currentEnabled ? i.hermesMemory.enabled : i.hermesMemory.disabled}
          </span>
          <button
            onClick={() => void handleToggle(!currentEnabled)}
            style={{
              position: "relative",
              width: 36,
              height: 20,
              borderRadius: 10,
              border: "none",
              cursor: "pointer",
              background: currentEnabled ? "var(--accent)" : "var(--border-default)",
              transition: "background 0.2s",
            }}
          >
            <span style={{
              position: "absolute",
              top: 2,
              left: currentEnabled ? 18 : 2,
              width: 16,
              height: 16,
              borderRadius: "50%",
              background: "#fff",
              transition: "left 0.2s",
            }} />
          </button>
        </label>
      </div>

      {/* Editor area */}
      <div style={{ flex: 1, padding: "16px 20px", overflow: "auto" }}>
        {!loaded ? (
          <div className="loading-center">
            <div className="spinner" />
            <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
              {i.hermesMemory.loading}
            </span>
          </div>
        ) : (
          <MarkdownEditor
            value={content}
            onChange={setContent}
          />
        )}
      </div>

      {/* Footer */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 20px", borderTop: "1px solid var(--border-default)" }}>
        <span style={{ fontSize: 12, fontWeight: isOver ? 600 : 400, color: isOver ? "var(--error)" : "var(--text-muted)" }}>
          {charCount} / {currentLimit} {i.hermesMemory.chars}
          {isOver && ` — ${i.hermesMemory.overLimit}`}
        </span>
        <button
          onClick={() => void handleSave()}
          disabled={saving || !loaded}
          className="btn btn-primary btn-sm"
          style={{ display: "flex", alignItems: "center", gap: 6 }}
        >
          {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
          {i.hermesMemory.save}
        </button>
      </div>
    </div>
  );
}

export default HermesMemory;
