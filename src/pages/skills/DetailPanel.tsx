import { Check, Edit3, FileText, Trash2, Zap } from "lucide-react";

import MarkdownPreview from "../../components/MarkdownPreview";
import type { I18n, Locale } from "../../lib/i18n";
import type { DetectedTool } from "../../types/skills";
import type { Skill } from "./helpers";

interface SkillsDetailPanelProps {
  selectedSkill: Skill;
  skillContent: string | null;
  loadingContent: boolean;
  editingSkill: boolean;
  syncedSkills: Record<string, Set<string>>;
  setSyncedSkills: React.Dispatch<React.SetStateAction<Record<string, Set<string>>>>;
  tools: DetectedTool[];
  skillSyncMethod: string;
  handleToggleSkill: (skill: Skill) => void;
  handleDeleteSkill: (skill: Skill) => void;
  setEditingSkill: (value: boolean) => void;
  setSelectedSkill: (skill: Skill | null) => void;
  // mutations are typed as `any` here because their full input shapes leak
  // implementation details; the original parent passes the exact hook results.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  copySkillBetweenToolsMutation: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  removeSyncedSkillMutation: any;
  locale: Locale;
  i: I18n;
}

export default function SkillsDetailPanel(p: SkillsDetailPanelProps) {
  const {
    selectedSkill,
    skillContent,
    loadingContent,
    editingSkill,
    syncedSkills,
    setSyncedSkills,
    tools,
    skillSyncMethod,
    handleToggleSkill,
    handleDeleteSkill,
    setEditingSkill,
    setSelectedSkill,
    copySkillBetweenToolsMutation,
    removeSyncedSkillMutation,
    locale,
    i,
  } = p;
  return (
    <div style={{ width: 520, overflowY: "auto", flexShrink: 0 }}>
      <div className="section-card" style={{ position: "sticky", top: 0 }}>
        <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", marginBottom: 18 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <div
              className="icon-box"
              style={{ background: "var(--warning-subtle)", width: 42, height: 42, borderRadius: 8 }}
            >
              <Zap size={20} style={{ color: "var(--warning)" }} />
            </div>
            <div>
              <h3 style={{ fontSize: 15, fontWeight: 700 }}>{selectedSkill.name}</h3>
              {selectedSkill.plugin_id && (
                <span className="badge badge-muted" style={{ marginTop: 4 }}>
                  {selectedSkill.plugin_id}
                </span>
              )}
            </div>
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            {selectedSkill.file_path && !editingSkill && (
              <>
                <button
                  onClick={() => handleToggleSkill(selectedSkill)}
                  title={
                    selectedSkill.file_path.endsWith(".disabled")
                      ? locale === "zh"
                        ? "启用"
                        : "Enable"
                      : locale === "zh"
                        ? "禁用"
                        : "Disable"
                  }
                  className="btn btn-ghost btn-icon-sm"
                  style={{
                    color: selectedSkill.file_path.endsWith(".disabled") ? "var(--text-muted)" : "var(--success)",
                  }}
                >
                  <Check size={14} />
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => setEditingSkill(true)}
                  title={locale === "zh" ? "编辑" : "Edit"}
                >
                  <Edit3 size={14} />
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => handleDeleteSkill(selectedSkill)}
                  title={locale === "zh" ? "删除" : "Delete"}
                  style={{ color: "var(--danger)" }}
                >
                  <Trash2 size={14} />
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => setSelectedSkill(null)}
                  title={locale === "zh" ? "关闭" : "Close"}
                >
                  ×
                </button>
              </>
            )}
          </div>
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          {selectedSkill.description && (
            <div>
              <div className="field-label" style={{ marginBottom: 6 }}>
                {locale === "zh" ? "描述" : "Description"}
              </div>
              <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
                {selectedSkill.description}
              </p>
            </div>
          )}

          {selectedSkill.trigger_command && (
            <div>
              <div className="field-label" style={{ marginBottom: 6 }}>
                {locale === "zh" ? "触发命令" : "Trigger Command"}
              </div>
              <div className="code-block" style={{ fontSize: 12 }}>
                /{selectedSkill.trigger_command}
              </div>
            </div>
          )}

          {selectedSkill.file_path && (
            <div>
              <div className="field-label" style={{ marginBottom: 6 }}>
                {locale === "zh" ? "文件路径" : "File Path"}
              </div>
              <div className="code-block" style={{ fontSize: 11 }}>
                {selectedSkill.file_path}
              </div>
            </div>
          )}

          {/* 同步到其他工具 */}
          {selectedSkill.file_path && (
            <div>
              <div className="field-label" style={{ marginBottom: 8 }}>
                {locale === "zh" ? "同步到其他工具" : "Sync to other tools"}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {tools
                  .filter((t) => t.installed && t.id !== selectedSkill.tool_id)
                  .map((tool) => {
                    const isSynced = syncedSkills[selectedSkill.id]?.has(tool.id);
                    return (
                      <div
                        key={tool.id}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "space-between",
                          padding: "8px 10px",
                          background: "var(--bg-card)",
                          border: "1px solid var(--border-default)",
                          borderRadius: 4,
                        }}
                      >
                        <span style={{ fontSize: 12 }}>{tool.name}</span>
                        {isSynced ? (
                          <button
                            className="btn btn-ghost btn-xs"
                            style={{ color: "var(--danger)" }}
                            onClick={async () => {
                              const skillName = selectedSkill.file_path
                                ?.split(/[\\/]/)
                                .pop()
                                ?.replace(/\.disabled$/, "");
                              if (!skillName) return;
                              try {
                                await removeSyncedSkillMutation.mutateAsync({
                                  skillName,
                                  targetSkillsDir: tool.skills_dir,
                                });
                                setSyncedSkills((prev) => {
                                  const next = { ...prev };
                                  if (next[selectedSkill.id]) {
                                    const s = new Set(next[selectedSkill.id]);
                                    s.delete(tool.id);
                                    next[selectedSkill.id] = s;
                                  }
                                  return next;
                                });
                              } catch (error) {
                                console.error("Failed to remove synced skill", error);
                              }
                            }}
                          >
                            {locale === "zh" ? "取消同步" : "Unsync"}
                          </button>
                        ) : (
                          <button
                            className="btn btn-secondary btn-xs"
                            onClick={async () => {
                              if (!selectedSkill.file_path) return;
                              try {
                                await copySkillBetweenToolsMutation.mutateAsync({
                                  path: selectedSkill.file_path,
                                  targetSkillsDir: tool.skills_dir,
                                  method: skillSyncMethod,
                                });
                                setSyncedSkills((prev) => {
                                  const next = { ...prev };
                                  if (!next[selectedSkill.id]) next[selectedSkill.id] = new Set();
                                  next[selectedSkill.id] = new Set([...next[selectedSkill.id], tool.id]);
                                  return next;
                                });
                              } catch (error) {
                                console.error("Failed to copy skill", error);
                              }
                            }}
                          >
                            {locale === "zh" ? "同步" : "Sync"}
                          </button>
                        )}
                      </div>
                    );
                  })}
                {tools.filter((t) => t.installed && t.id !== selectedSkill.tool_id).length === 0 && (
                  <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
                    {locale === "zh" ? "没有其他已安装的工具" : "No other installed tools"}
                  </p>
                )}
              </div>
            </div>
          )}

          {/* 内容预览 */}
          <div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                marginBottom: 8,
              }}
            >
              <div className="field-label" style={{ marginBottom: 0 }}>
                <FileText size={12} style={{ marginRight: 6, verticalAlign: "middle" }} />
                {locale === "zh" ? "内容预览" : "Content Preview"}
              </div>
            </div>
            {loadingContent ? (
              <div className="code-block" style={{ textAlign: "center", padding: 20, color: "var(--text-muted)" }}>
                {i.skills.loading}
              </div>
            ) : skillContent ? (
              <div
                className="markdown-preview"
                style={{ maxHeight: 500, overflowY: "auto", fontSize: 13, lineHeight: 1.8 }}
              >
                <MarkdownPreview content={skillContent} />
              </div>
            ) : (
              <div className="code-block" style={{ textAlign: "center", padding: 20, color: "var(--text-muted)" }}>
                {locale === "zh" ? "无可用内容" : "No content available"}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
