import { lazy, Suspense } from "react";
import { ArrowLeft, RotateCcw, Save, Zap } from "lucide-react";

import LoadingState from "../../components/states/LoadingState";
import type { I18n, Locale } from "../../lib/i18n";
import type { Skill } from "./helpers";

const MarkdownEditor = lazy(() => import("../../components/MarkdownEditor"));

interface SkillsEditingViewProps {
  selectedSkill: Skill;
  skillContent: string | null;
  editContent: string;
  setEditContent: (value: string) => void;
  hasEditChanges: boolean;
  handleSaveSkill: () => void;
  setEditingSkill: (value: boolean) => void;
  locale: Locale;
  i: I18n;
}

export default function SkillsEditingView({
  selectedSkill,
  skillContent,
  editContent,
  setEditContent,
  hasEditChanges,
  handleSaveSkill,
  setEditingSkill,
  locale,
  i,
}: SkillsEditingViewProps) {
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      {/* Header */}
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => setEditingSkill(false)}>
            <ArrowLeft size={16} />
          </button>
          <Zap size={18} style={{ color: "var(--warning)" }} />
          <h2 className="page-title" style={{ margin: 0 }}>
            {selectedSkill.name}
          </h2>
          {selectedSkill.file_path?.endsWith(".disabled") && (
            <span className="badge badge-muted" style={{ fontSize: 10 }}>
              {locale === "zh" ? "已禁用" : "Disabled"}
            </span>
          )}
          {hasEditChanges && <span className="badge badge-warning">{locale === "zh" ? "未保存" : "Unsaved"}</span>}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          {hasEditChanges && (
            <button className="btn btn-secondary btn-sm" onClick={() => setEditContent(skillContent || "")}>
              <RotateCcw size={14} />
              {locale === "zh" ? "撤销" : "Revert"}
            </button>
          )}
          <button className="btn btn-primary btn-sm" onClick={handleSaveSkill} disabled={!hasEditChanges}>
            <Save size={14} />
            {i.common.save}
          </button>
        </div>
      </div>

      {/* File Path */}
      {selectedSkill.file_path && (
        <div style={{ marginBottom: 16 }}>
          <div className="code-block" style={{ fontSize: 11 }}>
            {selectedSkill.file_path}
          </div>
        </div>
      )}

      {/* Editor */}
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        <Suspense fallback={<LoadingState />}>
          <MarkdownEditor value={editContent} onChange={setEditContent} minHeight={500} />
        </Suspense>
      </div>
    </div>
  );
}
