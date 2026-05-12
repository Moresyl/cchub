import { memo, useCallback, useState } from "react";
import { ChevronDown, File, Folder } from "lucide-react";
import type { FolderNode } from "../../types/skills";

const TreeNode = memo(function TreeNodeComponent({
  node,
  onSelect,
  selectedPath,
  depth = 0,
}: {
  node: FolderNode;
  onSelect: (path: string) => void;
  selectedPath: string | null;
  depth?: number;
}) {
  const [open, setOpen] = useState(depth < 1);
  const handleToggleOpen = useCallback(() => {
    setOpen((current) => !current);
  }, []);
  const handleSelectNode = useCallback(() => {
    onSelect(node.path);
  }, [node.path, onSelect]);

  if (node.is_dir) {
    return (
      <div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            padding: "4px 8px",
            paddingLeft: depth * 16 + 8,
            cursor: "pointer",
            borderRadius: 4,
            fontSize: 13,
            color: "var(--text-secondary)",
          }}
          onClick={handleToggleOpen}
        >
          <ChevronDown
            size={13}
            style={{ transform: open ? "none" : "rotate(-90deg)", transition: "transform 0.15s", flexShrink: 0 }}
          />
          <Folder size={14} style={{ color: "var(--text-secondary)", flexShrink: 0 }} />
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{node.name}</span>
        </div>
        {open &&
          node.children.map((child) => (
            <TreeNode key={child.path} node={child} onSelect={onSelect} selectedPath={selectedPath} depth={depth + 1} />
          ))}
      </div>
    );
  }

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 6,
        padding: "4px 8px",
        paddingLeft: depth * 16 + 28,
        cursor: "pointer",
        borderRadius: 4,
        fontSize: 13,
        color: selectedPath === node.path ? "var(--text-primary)" : "var(--text-muted)",
        background: selectedPath === node.path ? "var(--bg-card-hover)" : "transparent",
      }}
      onClick={handleSelectNode}
    >
      <File size={13} style={{ flexShrink: 0 }} />
      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{node.name}</span>
    </div>
  );
});

export default TreeNode;
