import { memo, type ReactNode } from "react";
import {
  ChevronDown,
  ChevronRight,
  File,
  FileCode2,
  FileJson,
  Folder,
  FolderOpen,
  type LucideIcon,
} from "lucide-react";
import type { FolderNode } from "../types/skills";

type EditorLanguage = "json" | "markdown" | "yaml" | "toml" | "text";

interface ConfigFilesTreePanelProps {
  title: string;
  rootPath: string;
  loading: boolean;
  tree: FolderNode | null | undefined;
  activeFile: string | null;
  expanded: Record<string, boolean>;
  noRootLabel: string;
  noRootTip: string;
  onToggleExpand: (path: string) => void | Promise<void>;
  onOpenFile: (path: string) => void | Promise<void>;
}

function detectLanguage(path: string): EditorLanguage {
  const lower = path.toLowerCase();
  if (lower.endsWith(".json")) return "json";
  if (lower.endsWith(".toml")) return "toml";
  if (lower.endsWith(".yaml") || lower.endsWith(".yml")) return "yaml";
  if (lower.endsWith(".md") || lower.endsWith(".mdx") || lower.endsWith(".markdown")) return "markdown";
  return "text";
}

function fileIcon(path: string): LucideIcon {
  const language = detectLanguage(path);
  switch (language) {
    case "json":
      return FileJson;
    case "toml":
    case "yaml":
      return FileCode2;
    default:
      return File;
  }
}

function ConfigFilesTreePanelComponent({
  title,
  rootPath,
  loading,
  tree,
  activeFile,
  expanded,
  noRootLabel,
  noRootTip,
  onToggleExpand,
  onOpenFile,
}: ConfigFilesTreePanelProps) {
  const renderNode = (node: FolderNode, depth = 0): ReactNode => {
    const isExpanded = expanded[node.path] ?? depth < 1;
    const isSelected = activeFile === node.path;

    if (node.is_dir) {
      return (
        <div key={node.path}>
          <button
            className="btn btn-ghost"
            onClick={() => onToggleExpand(node.path)}
            style={{
              width: "100%",
              justifyContent: "flex-start",
              padding: "6px 8px",
              paddingLeft: 8 + depth * 14,
              borderRadius: 6,
              color: "var(--text-secondary)",
              gap: 8,
            }}
          >
            {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            {isExpanded ? <FolderOpen size={14} /> : <Folder size={14} />}
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{node.name}</span>
          </button>
          {isExpanded && node.children.map((child) => renderNode(child, depth + 1))}
        </div>
      );
    }

    const Icon = fileIcon(node.path);
    return (
      <button
        key={node.path}
        className="btn btn-ghost"
        onClick={() => onOpenFile(node.path)}
        style={{
          width: "100%",
          justifyContent: "flex-start",
          padding: "6px 8px",
          paddingLeft: 36 + depth * 14,
          borderRadius: 6,
          gap: 8,
          color: isSelected ? "var(--text-primary)" : "var(--text-secondary)",
          background: isSelected ? "var(--bg-card-hover)" : "transparent",
          border: isSelected ? "1px solid var(--border-default)" : "1px solid transparent",
        }}
        title={node.path}
      >
        <Icon size={14} />
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{node.name}</span>
      </button>
    );
  };

  return (
    <div className="card" style={{ minHeight: 0, overflow: "hidden", display: "flex", flexDirection: "column" }}>
      <div style={{ padding: "14px 16px", borderBottom: "1px solid var(--border-default)" }}>
        <div style={{ fontSize: 13, fontWeight: 700 }}>{title}</div>
        <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 4 }}>
          {rootPath}
        </div>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 10 }}>
        {loading ? (
          <div className="loading-center" style={{ height: "100%" }}>
            <div className="spinner" />
          </div>
        ) : tree ? (
          renderNode(tree)
        ) : (
          <div className="empty-state" style={{ minHeight: 260 }}>
            <div className="empty-icon">
              <FolderOpen size={28} style={{ color: "var(--text-muted)" }} />
            </div>
            <p style={{ fontSize: 14, fontWeight: 600, color: "var(--text-secondary)" }}>{noRootLabel}</p>
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 8, maxWidth: 240 }}>{noRootTip}</p>
          </div>
        )}
      </div>
    </div>
  );
}

export default memo(ConfigFilesTreePanelComponent);
