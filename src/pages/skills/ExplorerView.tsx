import { X } from "lucide-react";

import type { FolderNode } from "../../types/skills";
import type { I18n, Locale } from "../../lib/i18n";
import TreeNode from "./TreeNode";

interface SkillsExplorerViewProps {
  folderTree: FolderNode | null;
  explorerPreview: string | null;
  explorerFile: string | null;
  previewExplorerFile: (path: string) => void;
  setShowExplorer: (value: boolean) => void;
  locale: Locale;
  i: I18n;
}

export default function SkillsExplorerView({
  folderTree,
  explorerPreview,
  explorerFile,
  previewExplorerFile,
  setShowExplorer,
  locale,
  i,
}: SkillsExplorerViewProps) {
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => setShowExplorer(false)}
            title={locale === "zh" ? "返回" : "Back"}
          >
            <X size={18} />
          </button>
          <div>
            <h2 className="page-title">{i.skills.explorerTitle}</h2>
            <p className="page-subtitle">
              {locale === "zh" ? "浏览技能目录和文件" : "Browse skill directory and files"}
            </p>
          </div>
        </div>
      </div>

      <div style={{ flex: 1, display: "flex", gap: 16, minHeight: 0 }}>
        <div
          style={{ width: 280, overflowY: "auto", borderRight: "1px solid var(--border-default)", paddingRight: 16 }}
        >
          {folderTree ? (
            <TreeNode node={folderTree} onSelect={previewExplorerFile} selectedPath={explorerFile} />
          ) : (
            <p style={{ fontSize: 13, color: "var(--text-muted)", padding: 20, textAlign: "center" }}>
              {locale === "zh" ? "目录不存在" : "Directory not found"}
            </p>
          )}
        </div>
        <div style={{ flex: 1, overflowY: "auto" }}>
          {explorerPreview ? (
            <div className="code-block" style={{ height: "100%", fontSize: 11, lineHeight: 1.7 }}>
              {explorerPreview}
            </div>
          ) : (
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                height: "100%",
                color: "var(--text-muted)",
                fontSize: 13,
              }}
            >
              {i.skills.noPreview}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
