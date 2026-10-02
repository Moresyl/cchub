import { memo } from "react";
import { Bot, Code2, FolderOpen, Globe, Monitor, Sparkles, Terminal, type LucideIcon } from "lucide-react";
import { Button } from "./ui/button";

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
  pi: Terminal,
  grokbuild: Terminal,
  mcode: Bot,
  "claude-desktop": Monitor,
};

function ConfigFilesRootTabsComponent({ roots, activeRoot, onSelectRoot }: ConfigFilesRootTabsProps) {
  return (
    <div className="config-file-root-tabs">
      {roots.map((root) => {
        const Icon = ROOT_ICONS[root.id] || FolderOpen;
        return (
          <Button
            key={root.id}
            variant={activeRoot === root.id ? "default" : "secondary"}
            aria-pressed={activeRoot === root.id}
            disabled={!root.exists}
            onClick={() => onSelectRoot(root.id)}
            style={{ opacity: root.exists ? 1 : 0.45 }}
            title={root.path}
          >
            <Icon size={14} />
            {root.name}
          </Button>
        );
      })}
    </div>
  );
}

export default memo(ConfigFilesRootTabsComponent);
