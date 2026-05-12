import { lazy, Suspense } from "react";
import { Save, Wand2, X } from "lucide-react";

import { showToast } from "../../components/Toast";
import type { I18n } from "../../lib/i18n";
import type { McpServer } from "./helpers";

const CodeEditor = lazy(() => import("../../components/CodeEditor"));

interface McpServerEditViewProps {
  selected: McpServer;
  i: I18n;
  zh: boolean;
  editCommand: string;
  setEditCommand: (value: string) => void;
  editArgs: string;
  setEditArgs: (value: string) => void;
  editEnv: string;
  setEditEnv: (value: string) => void;
  setEditing: (value: boolean) => void;
  handleSave: () => void;
}

export default function McpServerEditView({
  selected,
  i,
  zh,
  editCommand,
  setEditCommand,
  editArgs,
  setEditArgs,
  editEnv,
  setEditEnv,
  setEditing,
  handleSave,
}: McpServerEditViewProps) {
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => setEditing(false)} title={i.mcp.cancel}>
            <X size={18} />
          </button>
          <div>
            <h2 className="page-title">{selected.name}</h2>
            <p className="page-subtitle">{zh ? "编辑 MCP 服务器配置" : "Edit MCP server configuration"}</p>
          </div>
        </div>
      </div>

      <div
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          gap: 20,
          paddingBottom: 20,
        }}
      >
        <div>
          <span className="field-label">{i.mcp.command}</span>
          <input
            className="input"
            style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
            value={editCommand}
            onChange={(e) => setEditCommand(e.target.value)}
            placeholder="npx, node, python..."
          />
        </div>

        <div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <span className="field-label" style={{ marginBottom: 0 }}>
              {i.mcp.arguments}
            </span>
            <button
              className="btn btn-ghost btn-icon-sm"
              title="Format"
              onClick={() => {
                try {
                  setEditArgs(JSON.stringify(JSON.parse(editArgs), null, 2));
                } catch (error) {
                  showToast("error", String(error));
                }
              }}
            >
              <Wand2 size={12} />
            </button>
          </div>
          <Suspense fallback={null}>
            <CodeEditor value={editArgs} onChange={setEditArgs} language="json" minHeight={160} />
          </Suspense>
        </div>

        <div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <span className="field-label" style={{ marginBottom: 0 }}>
              {i.mcp.environment}
            </span>
            <button
              className="btn btn-ghost btn-icon-sm"
              title="Format"
              onClick={() => {
                try {
                  setEditEnv(JSON.stringify(JSON.parse(editEnv), null, 2));
                } catch (error) {
                  showToast("error", String(error));
                }
              }}
            >
              <Wand2 size={12} />
            </button>
          </div>
          <Suspense fallback={null}>
            <CodeEditor value={editEnv} onChange={setEditEnv} language="json" minHeight={160} />
          </Suspense>
        </div>
      </div>

      <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <button className="btn btn-secondary btn-sm" onClick={() => setEditing(false)}>
          {i.mcp.cancel}
        </button>
        <button className="btn btn-primary btn-sm" onClick={handleSave} style={{ gap: 6 }}>
          <Save size={14} />
          {i.mcp.save}
        </button>
      </div>
    </div>
  );
}
