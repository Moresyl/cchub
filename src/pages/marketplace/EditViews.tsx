/* eslint-disable @typescript-eslint/no-explicit-any */
import { Suspense, lazy } from "react";
import { ArrowLeft, RotateCcw, Save, Zap } from "lucide-react";

import { t } from "../../lib/i18n";
import type { InstalledMcpServer, SkillEntry } from "./helpers";

import McpServerEditView from "../mcp-servers/EditView";

const MarkdownEditor = lazy(() => import("../../components/MarkdownEditor"));

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
  saving?: boolean;
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
  return (
    <McpServerEditView
      selected={props.editingMcp}
      i={t()}
      zh={props.locale === "zh"}
      editCommand={props.editCommand}
      setEditCommand={props.setEditCommand}
      editArgs={props.editArgs}
      setEditArgs={props.setEditArgs}
      editEnv={props.editEnv}
      setEditEnv={props.setEditEnv}
      setEditing={() => props.setEditingMcp(null)}
      handleSave={props.handleSaveMcpConfig}
      saving={props.saving}
      saveDisabled={!props.hasMcpChanges}
      onRevert={
        props.hasMcpChanges
          ? () => {
              props.setEditCommand(props.originalMcpCommand);
              props.setEditArgs(props.originalMcpArgs);
              props.setEditEnv(props.originalMcpEnv);
            }
          : undefined
      }
    />
  );
}
