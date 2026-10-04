import { useId } from "react";
import { Save, Wand2, X } from "lucide-react";

import { showToast } from "../../components/Toast";
import type { I18n } from "../../lib/i18n";
import type { McpServer } from "./helpers";
import CodeEditor from "../../components/DeferredCodeEditor";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";

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
  saving?: boolean;
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
  saving = false,
}: McpServerEditViewProps) {
  const commandId = useId();
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => setEditing(false)}
            disabled={saving}
            title={i.mcp.cancel}
            aria-label={i.mcp.cancel}
          >
            <X size={18} />
          </Button>
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
          <label className="field-label" htmlFor={commandId}>
            {i.mcp.command}
          </label>
          <Input
            id={commandId}
            style={{ fontFamily: "var(--font-code)", fontSize: 12 }}
            value={editCommand}
            disabled={saving}
            onChange={(e) => setEditCommand(e.target.value)}
            placeholder="npx, node, python..."
          />
        </div>

        <div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <span className="field-label" style={{ marginBottom: 0 }}>
              {i.mcp.arguments}
            </span>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              title={zh ? "格式化参数" : "Format arguments"}
              disabled={saving}
              aria-label={zh ? "格式化参数" : "Format arguments"}
              onClick={() => {
                try {
                  setEditArgs(JSON.stringify(JSON.parse(editArgs), null, 2));
                } catch {
                  showToast(
                    "error",
                    zh ? "参数 JSON 格式不正确，内容已保留。" : "Invalid arguments JSON. Your content is preserved.",
                  );
                }
              }}
            >
              <Wand2 size={12} />
            </Button>
          </div>
          <CodeEditor
            value={editArgs}
            readOnly={saving}
            onChange={setEditArgs}
            ariaLabel={i.mcp.arguments}
            language="json"
            minHeight={160}
          />
        </div>

        <div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <span className="field-label" style={{ marginBottom: 0 }}>
              {i.mcp.environment}
            </span>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              title={zh ? "格式化环境变量" : "Format environment"}
              disabled={saving}
              aria-label={zh ? "格式化环境变量" : "Format environment"}
              onClick={() => {
                try {
                  setEditEnv(JSON.stringify(JSON.parse(editEnv), null, 2));
                } catch {
                  showToast(
                    "error",
                    zh
                      ? "环境变量 JSON 格式不正确，内容已保留。"
                      : "Invalid environment JSON. Your content is preserved.",
                  );
                }
              }}
            >
              <Wand2 size={12} />
            </Button>
          </div>
          <CodeEditor
            value={editEnv}
            readOnly={saving}
            onChange={setEditEnv}
            ariaLabel={i.mcp.environment}
            language="json"
            minHeight={160}
          />
        </div>
      </div>

      <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <Button type="button" variant="secondary" disabled={saving} onClick={() => setEditing(false)}>
          {i.mcp.cancel}
        </Button>
        <Button type="button" onClick={handleSave} disabled={saving} aria-busy={saving}>
          <Save size={14} />
          {saving ? (zh ? "保存中…" : "Saving…") : i.mcp.save}
        </Button>
      </div>
    </div>
  );
}
