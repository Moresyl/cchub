import { memo } from "react";
import { Bot, Code2, FolderOpen, Globe, Monitor, Sparkles, Terminal, type LucideIcon } from "lucide-react";

interface ConfigRootTabItem {
  id: string;
  name: string;
  path: string;
  exists: boolean;
}

interface ConfigFilesRootTabsProps {
  roots: ConfigRootTabItem[];
  activeRoot: string;
  onSelectRoot: (rootId: string) => void | Promise<void>;
}

const ROOT_ICONS: Record<string, LucideIcon> = {
  claude: Terminal,
  codex: Code2,
  gemini: Sparkles,
  opencode: Globe,
  openclaw: Monitor,
  hermes: Bot,
};

function ConfigFilesRootTabsComponent({
  roots,
  activeRoot,
  onSelectRoot,
}: ConfigFilesRootTabsProps) {
  return (
    <div style={{ display: "flex", gap: 8, marginBottom: 16, flexWrap: "wrap" }}>
      {roots.map((root) => {
        const Icon = ROOT_ICONS[root.id] || FolderOpen;
        return (
          <button
            key={root.id}
            className={`btn btn-sm ${activeRoot === root.id ? "btn-primary" : "btn-secondary"}`}
            disabled={!root.exists}
            onClick={() => onSelectRoot(root.id)}
            style={{ opacity: root.exists ? 1 : 0.45 }}
            title={root.path}
          >
            <Icon size={14} />
            {root.name}
          </button>
        );
      })}
    </div>
  );
}

export default memo(ConfigFilesRootTabsComponent);
