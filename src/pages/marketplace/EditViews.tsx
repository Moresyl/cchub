/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense, lazy } from "react";
import { ArrowLeft, RotateCcw, Save, Wand2, Zap } from "lucide-react";

import { showToast } from "../../components/Toast";
import { t } from "../../lib/i18n";
import type { InstalledMcpServer, SkillEntry } from "./helpers";

const MarkdownEditor = lazy(() => import("../../components/MarkdownEditor"));
const CodeEditor = lazy(() => import("../../components/CodeEditor"));

interface SkillEditViewProps {
  locale: string;
  editingSkill: SkillEntry;
  skillContent: string;
  editSkillContent: string;
  hasSkillChanges: boolean;
  setEditingSkill: (skill: SkillEntry | null) => void;
  setEditSkillContent: (content: string) => void;
  handleSaveSkillContent: () => void;
}

export function SkillEditView(props: SkillEditViewProps) {
  const { locale, editingSkill, skillContent, editSkillContent, hasSkillChanges } = props;
  const i = t();
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => props.setEditingSkill(null)}>
            <ArrowLeft size={16} />
          </button>
          <Zap size={18} style={{ color: "var(--warning)" }} />
          <h2 className="page-title" style={{ margin: 0 }}>
            {editingSkill.name}
          </h2>
          {hasSkillChanges && <span className="badge badge-warning">{locale === "zh" ? "未保存" : "Unsaved"}</span>}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {hasSkillChanges && (
            <button className="btn btn-secondary btn-sm" onClick={() => props.setEditSkillContent(skillContent)}>
              <RotateCcw size={14} />
              {locale === "zh" ? "撤销" : "Revert"}
            </button>
          )}
          <button className="btn btn-primary btn-sm" onClick={props.handleSaveSkillContent} disabled={!hasSkillChanges}>
            <Save size={14} />
            {i.common.save}
          </button>
        </div>
      </div>

      {editingSkill.file_path && (
        <div style={{ marginBottom: 16 }}>
          <div className="code-block" style={{ fontSize: 11 }}>
            {editingSkill.file_path}
          </div>
        </div>
      )}

      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        <Suspense
          fallback={
            <div style={{ display: "flex", alignItems: "center", gap: 10, padding: 40, justifyContent: "center" }}>
              <div className="spinner" style={{ width: 18, height: 18 }} />
            </div>
          }
        >
          <MarkdownEditor value={editSkillContent} onChange={props.setEditSkillContent} minHeight={500} />
        </Suspense>
      </div>
    </div>
  );
}

interface McpEditViewProps {
  locale: string;
  editingMcp: InstalledMcpServer;
  editCommand: string;
  editArgs: string;
  editEnv: string;
  originalMcpCommand: string;
  originalMcpArgs: string;
  originalMcpEnv: string;
  hasMcpChanges: boolean;
  setEditingMcp: (mcp: InstalledMcpServer | null) => void;
  setEditCommand: (v: string) => void;
  setEditArgs: (v: string) => void;
  setEditEnv: (v: string) => void;
  handleSaveMcpConfig: () => void;
}

export function McpEditView(props: McpEditViewProps) {
  const { locale, editingMcp, editCommand, editArgs, editEnv, hasMcpChanges } = props;
  const i = t();
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => props.setEditingMcp(null)}
            title={locale === "zh" ? "返回" : "Back"}
          >
            <ArrowLeft size={16} />
          </button>
          <div>
            <h2 className="page-title">{editingMcp.name}</h2>
            <p className="page-subtitle">{locale === "zh" ? "编辑 MCP 服务器配置" : "Edit MCP server configuration"}</p>
          </div>
        </div>
        {hasMcpChanges && <span className="badge badge-warning">{locale === "zh" ? "未保存" : "Unsaved"}</span>}
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
          <span className="field-label">{locale === "zh" ? "命令" : "Command"}</span>
          <input
            className="input"
            style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
            value={editCommand}
            onChange={(e) => props.setEditCommand(e.target.value)}
            placeholder="npx, node, python..."
          />
        </div>

        <div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <span className="field-label" style={{ marginBottom: 0 }}>
              {locale === "zh" ? "参数" : "Arguments"}
            </span>
            <button
              className="btn btn-ghost btn-icon-sm"
              title="Format"
              onClick={() => {
                try {
                  props.setEditArgs(JSON.stringify(JSON.parse(editArgs), null, 2));
                } catch (error) {
                  showToast("error", String(error));
                }
              }}
            >
              <Wand2 size={12} />
            </button>
          </div>
          <CodeEditor value={editArgs} onChange={props.setEditArgs} language="json" minHeight={160} />
        </div>

        <div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
            <span className="field-label" style={{ marginBottom: 0 }}>
              {locale === "zh" ? "环境变量" : "Environment"}
            </span>
            <button
              className="btn btn-ghost btn-icon-sm"
              title="Format"
              onClick={() => {
                try {
                  props.setEditEnv(JSON.stringify(JSON.parse(editEnv), null, 2));
                } catch (error) {
                  showToast("error", String(error));
                }
              }}
            >
              <Wand2 size={12} />
            </button>
          </div>
          <CodeEditor value={editEnv} onChange={props.setEditEnv} language="json" minHeight={160} />
        </div>
      </div>

      <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        {hasMcpChanges && (
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => {
              props.setEditCommand(props.originalMcpCommand);
              props.setEditArgs(props.originalMcpArgs);
              props.setEditEnv(props.originalMcpEnv);
            }}
          >
            <RotateCcw size={14} />
            {locale === "zh" ? "撤销" : "Revert"}
          </button>
        )}
        <button className="btn btn-primary btn-sm" onClick={props.handleSaveMcpConfig} disabled={!hasMcpChanges}>
          <Save size={14} />
          {i.common.save}
        </button>
      </div>
    </div>
  );
}
