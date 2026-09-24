import { useEffect, useState } from "react";
import { Check, Edit3, FileText, Trash2, X, Zap } from "lucide-react";

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
  // Mutation input shapes are owned by the parent hooks.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  copySkillBetweenToolsMutation: any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  removeSyncedSkillMutation: any;
  locale: Locale;
  i: I18n;
}

type DetailTab = "overview" | "content" | "sync";

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
  const [activeTab, setActiveTab] = useState<DetailTab>("overview");
  const syncTargets = tools.filter((tool) => tool.installed && tool.id !== selectedSkill.tool_id);
  const zh = locale === "zh";

  useEffect(() => {
    setActiveTab("overview");
  }, [selectedSkill.id]);

  const removeSync = async (tool: DetectedTool) => {
    const skillName = selectedSkill.file_path
      ?.split(/[\\/]/)
      .pop()
      ?.replace(/\.disabled$/, "");
    if (!skillName) return;
    try {
      await removeSyncedSkillMutation.mutateAsync({ skillName, targetSkillsDir: tool.skills_dir });
      setSyncedSkills((current) => {
        const next = { ...current };
        const ids = new Set(next[selectedSkill.id]);
        ids.delete(tool.id);
        next[selectedSkill.id] = ids;
        return next;
      });
    } catch (error) {
      console.error("Failed to remove synced skill", error);
    }
  };

  const addSync = async (tool: DetectedTool) => {
    if (!selectedSkill.file_path) return;
    try {
      await copySkillBetweenToolsMutation.mutateAsync({
        path: selectedSkill.file_path,
        targetSkillsDir: tool.skills_dir,
        method: skillSyncMethod,
      });
      setSyncedSkills((current) => ({
        ...current,
        [selectedSkill.id]: new Set([...(current[selectedSkill.id] || []), tool.id]),
      }));
    } catch (error) {
      console.error("Failed to copy skill", error);
    }
  };

  return (
    <div className="entity-detail">
      <header className="entity-detail-header">
        <div className="entity-detail-heading">
          <div className="icon-box size-9 bg-[var(--warning-subtle)]">
            <Zap size={17} className="text-[var(--warning)]" />
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="entity-detail-title">{selectedSkill.name}</h3>
            <p className="mt-0.5 truncate text-[11px] text-[var(--text-muted)]">
              {selectedSkill.plugin_id || (zh ? "本地技能" : "Local skill")}
            </p>
          </div>
        </div>
        <div className="entity-detail-actions">
          {selectedSkill.file_path && !editingSkill && (
            <>
              <button
                onClick={() => handleToggleSkill(selectedSkill)}
                title={
                  selectedSkill.file_path.endsWith(".disabled") ? (zh ? "启用" : "Enable") : zh ? "禁用" : "Disable"
                }
                className="btn btn-ghost btn-icon-sm"
              >
                <Check size={14} />
              </button>
              <button
                className="btn btn-ghost btn-icon-sm"
                onClick={() => setEditingSkill(true)}
                title={zh ? "编辑" : "Edit"}
              >
                <Edit3 size={14} />
              </button>
              <button
                className="btn btn-danger-ghost btn-icon-sm"
                onClick={() => handleDeleteSkill(selectedSkill)}
                title={zh ? "删除" : "Delete"}
              >
                <Trash2 size={14} />
              </button>
            </>
          )}
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => setSelectedSkill(null)}
            title={zh ? "关闭" : "Close"}
          >
            <X size={15} />
          </button>
        </div>
      </header>

      <div className="entity-detail-tabs" role="tablist" aria-label={zh ? "技能详情视图" : "Skill detail views"}>
        {(
          [
            ["overview", zh ? "概览" : "Overview"],
            ["content", zh ? "内容" : "Content"],
            ["sync", zh ? "同步" : "Sync"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            className={`entity-detail-tab ${activeTab === id ? "entity-detail-tab-active" : ""}`}
            role="tab"
            aria-selected={activeTab === id}
            onClick={() => setActiveTab(id)}
          >
            {label}
          </button>
        ))}
      </div>

      <div className="entity-detail-scroll">
        {activeTab === "overview" && (
          <div className="detail-section-stack">
            {selectedSkill.description && (
              <section>
                <div className="field-label">{zh ? "描述" : "Description"}</div>
                <p className="text-[13px] leading-6 text-[var(--text-secondary)]">{selectedSkill.description}</p>
              </section>
            )}
            {selectedSkill.trigger_command && (
              <section>
                <div className="field-label">{zh ? "触发命令" : "Trigger command"}</div>
                <div className="code-block text-xs">/{selectedSkill.trigger_command}</div>
              </section>
            )}
            {selectedSkill.file_path && (
              <section>
                <div className="field-label">{zh ? "文件路径" : "File path"}</div>
                <div className="code-block break-all text-[11px]">{selectedSkill.file_path}</div>
              </section>
            )}
          </div>
        )}

        {activeTab === "content" && (
          <section className="detail-section-stack">
            <div className="field-label flex items-center gap-1.5">
              <FileText size={12} />
              {zh ? "内容预览" : "Content preview"}
            </div>
            {loadingContent ? (
              <div className="code-block py-8 text-center text-[var(--text-muted)]">{i.skills.loading}</div>
            ) : skillContent ? (
              <div className="markdown-preview text-[13px] leading-7">
                <MarkdownPreview content={skillContent} />
              </div>
            ) : (
              <div className="code-block py-8 text-center text-[var(--text-muted)]">
                {zh ? "无可用内容" : "No content available"}
              </div>
            )}
          </section>
        )}

        {activeTab === "sync" && (
          <section className="detail-section-stack">
            <div>
              <div className="field-label">{zh ? "同步到其他工具" : "Sync to other tools"}</div>
              <p className="text-xs leading-5 text-[var(--text-muted)]">
                {zh ? "选择要共享此技能的已安装工具。" : "Choose which installed tools can use this skill."}
              </p>
            </div>
            <div className="detail-row-list">
              {syncTargets.map((tool) => {
                const isSynced = syncedSkills[selectedSkill.id]?.has(tool.id);
                return (
                  <div key={tool.id} className="detail-row">
                    <span className="truncate text-[13px]">{tool.name}</span>
                    <button
                      className={`btn btn-xs ${isSynced ? "btn-ghost text-[var(--danger)]" : "btn-secondary"}`}
                      onClick={() => void (isSynced ? removeSync(tool) : addSync(tool))}
                    >
                      {isSynced ? (zh ? "取消同步" : "Unsync") : zh ? "同步" : "Sync"}
                    </button>
                  </div>
                );
              })}
              {syncTargets.length === 0 && (
                <p className="py-8 text-center text-xs text-[var(--text-muted)]">
                  {zh ? "没有其他已安装的工具" : "No other installed tools"}
                </p>
              )}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
