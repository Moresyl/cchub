import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { Command } from "cmdk";
import {
  Activity,
  ArrowRightLeft,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  LayoutDashboard,
  Layers,
  Monitor,
  Plug,
  Search,
  Settings,
  Shield,
  Store,
  Wrench,
  Webhook,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { t, getLocale } from "../lib/i18n";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  navigate: (path: string) => void;
}

interface PaletteItem {
  id: string;
  label: string;
  icon: LucideIcon;
  group: string;
  keywords: string[];
  value: string;
  action: PaletteAction;
}

type PaletteAction =
  | { type: "navigate"; path: string }
  | { type: "reload" }
  | { type: "dispatch"; eventName: string };

interface CommandPaletteItemRowProps {
  item: PaletteItem;
  onSelect: (action: PaletteAction) => void;
}

const PAGE_ICONS: Record<string, LucideIcon> = {
  "/": LayoutDashboard,
  "/mcp-servers": Plug,
  "/mcp-clients": Monitor,
  "/logs": Activity,
  "/skills": Zap,
  "/workflows": GitBranch,
  "/marketplace": Store,
  "/hooks": Webhook,
  "/workspaces": Layers,
  "/profiles": ArrowRightLeft,
  "/sessions": History,
  "/claude-md": FileText,
  "/config-files": FolderOpen,
  "/tools": Wrench,
  "/security": Shield,
  "/settings": Settings,
};

function CommandPaletteItemRowComponent({
  item,
  onSelect,
}: CommandPaletteItemRowProps) {
  const Icon = item.icon;
  const handleSelect = useCallback(() => {
    onSelect(item.action);
  }, [item.action, onSelect]);

  return (
    <Command.Item
      keywords={item.keywords}
      value={item.value}
      onSelect={handleSelect}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 12px",
        borderRadius: 10,
        cursor: "pointer",
        fontSize: 13,
      }}
    >
      <div
        className="icon-box"
        style={{ width: 30, height: 30, borderRadius: 8, flexShrink: 0, background: "var(--bg-elevated)" }}
      >
        <Icon size={15} />
      </div>
      <span>{item.label}</span>
    </Command.Item>
  );
}

const CommandPaletteItemRow = memo(CommandPaletteItemRowComponent);

function CommandPaletteComponent({
  open,
  onOpenChange,
  navigate,
}: CommandPaletteProps) {
  const [search, setSearch] = useState("");
  const i = t();
  const locale = getLocale();
  const uiText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);

  useEffect(() => {
    if (!open) {
      setSearch("");
    }
  }, [open]);

  const pageGroupLabel = uiText("页面", "Pages", "ページ");
  const actionGroupLabel = uiText("操作", "Actions", "操作");
  const dialogLabel = uiText("全局命令面板", "Global Command Palette", "グローバルコマンドパレット");
  const searchPlaceholder = uiText("搜索页面或动作...", "Search pages or actions...", "ページや操作を検索...");
  const emptyLabel = uiText("没有匹配项", "No results found", "一致する項目がありません");

  const pageItems = useMemo<PaletteItem[]>(
    () => [
      { id: "page-dashboard", label: i.nav.dashboard, icon: PAGE_ICONS["/"], group: pageGroupLabel, keywords: ["dashboard", "home"], value: `${i.nav.dashboard} dashboard home`, action: { type: "navigate", path: "/" } },
      { id: "page-mcp-servers", label: i.nav.mcpServers, icon: PAGE_ICONS["/mcp-servers"], group: pageGroupLabel, keywords: ["mcp", "servers"], value: `${i.nav.mcpServers} mcp servers`, action: { type: "navigate", path: "/mcp-servers" } },
      { id: "page-mcp-clients", label: i.nav.mcpClients, icon: PAGE_ICONS["/mcp-clients"], group: pageGroupLabel, keywords: ["mcp", "clients"], value: `${i.nav.mcpClients} mcp clients`, action: { type: "navigate", path: "/mcp-clients" } },
      { id: "page-logs", label: i.nav.logs, icon: PAGE_ICONS["/logs"], group: pageGroupLabel, keywords: ["logs", "activity"], value: `${i.nav.logs} logs activity`, action: { type: "navigate", path: "/logs" } },
      { id: "page-skills", label: i.nav.skills, icon: PAGE_ICONS["/skills"], group: pageGroupLabel, keywords: ["skills", "plugins"], value: `${i.nav.skills} skills plugins`, action: { type: "navigate", path: "/skills" } },
      { id: "page-workflows", label: i.nav.workflows, icon: PAGE_ICONS["/workflows"], group: pageGroupLabel, keywords: ["workflows", "templates"], value: `${i.nav.workflows} workflows templates`, action: { type: "navigate", path: "/workflows" } },
      { id: "page-marketplace", label: i.nav.marketplace, icon: PAGE_ICONS["/marketplace"], group: pageGroupLabel, keywords: ["market", "plugins"], value: `${i.nav.marketplace} market plugins`, action: { type: "navigate", path: "/marketplace" } },
      { id: "page-hooks", label: i.nav.hooks, icon: PAGE_ICONS["/hooks"], group: pageGroupLabel, keywords: ["hooks"], value: `${i.nav.hooks} hooks`, action: { type: "navigate", path: "/hooks" } },
      { id: "page-workspaces", label: i.nav.workspaces, icon: PAGE_ICONS["/workspaces"], group: pageGroupLabel, keywords: ["workspaces"], value: `${i.nav.workspaces} workspaces`, action: { type: "navigate", path: "/workspaces" } },
      { id: "page-profiles", label: i.nav.profiles, icon: PAGE_ICONS["/profiles"], group: pageGroupLabel, keywords: ["profiles", "providers"], value: `${i.nav.profiles} profiles providers`, action: { type: "navigate", path: "/profiles" } },
      { id: "page-sessions", label: i.nav.sessions, icon: PAGE_ICONS["/sessions"], group: pageGroupLabel, keywords: ["sessions", "history"], value: `${i.nav.sessions} sessions history`, action: { type: "navigate", path: "/sessions" } },
      { id: "page-claude-md", label: i.nav.claudeMd, icon: PAGE_ICONS["/claude-md"], group: pageGroupLabel, keywords: ["claude", "docs"], value: `${i.nav.claudeMd} claude docs`, action: { type: "navigate", path: "/claude-md" } },
      { id: "page-config-files", label: i.nav.configFiles, icon: PAGE_ICONS["/config-files"], group: pageGroupLabel, keywords: ["config", "files"], value: `${i.nav.configFiles} config files`, action: { type: "navigate", path: "/config-files" } },
      { id: "page-tools", label: i.nav.tools, icon: PAGE_ICONS["/tools"], group: pageGroupLabel, keywords: ["tools"], value: `${i.nav.tools} tools`, action: { type: "navigate", path: "/tools" } },
      { id: "page-security", label: i.nav.security, icon: PAGE_ICONS["/security"], group: pageGroupLabel, keywords: ["security", "audit"], value: `${i.nav.security} security audit`, action: { type: "navigate", path: "/security" } },
      { id: "page-settings", label: i.nav.settings, icon: PAGE_ICONS["/settings"], group: pageGroupLabel, keywords: ["settings", "preferences"], value: `${i.nav.settings} settings preferences`, action: { type: "navigate", path: "/settings" } },
    ],
    [i.nav, pageGroupLabel],
  );

  const actionItems = useMemo<PaletteItem[]>(
    () => [
      {
        id: "action-refresh",
        label: uiText("刷新当前页面", "Refresh Current Page", "現在のページを更新"),
        icon: Activity,
        group: actionGroupLabel,
        keywords: ["refresh", "reload"],
        value: `${uiText("刷新当前页面", "Refresh Current Page", "現在のページを更新")} refresh reload`,
        action: { type: "reload" },
      },
      {
        id: "action-save",
        label: uiText("保存当前内容", "Save Current Content", "現在の内容を保存"),
        icon: FileText,
        group: actionGroupLabel,
        keywords: ["save", "write"],
        value: `${uiText("保存当前内容", "Save Current Content", "現在の内容を保存")} save write`,
        action: { type: "dispatch", eventName: "cchub-shortcut-save" },
      },
      {
        id: "action-new",
        label: uiText("新建当前页面项", "Create New Item", "新規項目を作成"),
        icon: Layers,
        group: actionGroupLabel,
        keywords: ["new", "create", "add"],
        value: `${uiText("新建当前页面项", "Create New Item", "新規項目を作成")} new create add`,
        action: { type: "dispatch", eventName: "cchub-shortcut-new" },
      },
      {
        id: "action-search",
        label: uiText("聚焦页面搜索", "Focus Page Search", "ページ検索にフォーカス"),
        icon: Search,
        group: actionGroupLabel,
        keywords: ["search", "find", "filter"],
        value: `${uiText("聚焦页面搜索", "Focus Page Search", "ページ検索にフォーカス")} search find filter`,
        action: { type: "dispatch", eventName: "cchub-shortcut-search" },
      },
      {
        id: "action-settings",
        label: uiText("打开设置", "Open Settings", "設定を開く"),
        icon: Settings,
        group: actionGroupLabel,
        keywords: ["settings", "preferences"],
        value: `${uiText("打开设置", "Open Settings", "設定を開く")} settings preferences`,
        action: { type: "navigate", path: "/settings" },
      },
    ],
    [actionGroupLabel, uiText],
  );

  const groupedItems = useMemo(() => {
    const all = [...pageItems, ...actionItems];
    const groups = new Map<string, PaletteItem[]>();

    for (const item of all) {
      if (!groups.has(item.group)) {
        groups.set(item.group, []);
      }
      groups.get(item.group)?.push(item);
    }

    return Array.from(groups.entries());
  }, [actionItems, pageItems]);

  const handleSelectAction = useCallback((action: PaletteAction) => {
    onOpenChange(false);

    if (action.type === "navigate") {
      navigate(action.path);
      return;
    }

    if (action.type === "reload") {
      window.location.reload();
      return;
    }

    window.dispatchEvent(new CustomEvent(action.eventName));
  }, [navigate, onOpenChange]);

  return (
    <Command.Dialog
      open={open}
      onOpenChange={onOpenChange}
      label={dialogLabel}
    >
      <Command
        value={search}
        onValueChange={setSearch}
        style={{
          width: "min(720px, calc(100vw - 32px))",
          background: "var(--bg-card)",
          border: "1px solid var(--border-default)",
          borderRadius: 16,
          overflow: "hidden",
          boxShadow: "0 28px 80px rgba(0, 0, 0, 0.35)",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "14px 16px", borderBottom: "1px solid var(--border-default)" }}>
          <Search size={16} style={{ color: "var(--text-muted)" }} />
          <Command.Input
            value={search}
            onValueChange={setSearch}
            placeholder={searchPlaceholder}
            style={{
              width: "100%",
              border: 0,
              outline: 0,
              background: "transparent",
              color: "var(--text-primary)",
              fontSize: 14,
            }}
          />
          <span className="badge badge-muted" style={{ fontSize: 10, flexShrink: 0 }}>Ctrl+K</span>
        </div>
        <Command.List style={{ maxHeight: 420, overflowY: "auto", padding: 8 }}>
          <Command.Empty style={{ padding: 20, fontSize: 13, color: "var(--text-muted)", textAlign: "center" }}>
            {emptyLabel}
          </Command.Empty>
          {groupedItems.map(([group, items]) => (
            <Command.Group key={group} heading={group}>
              {items.map((item) => (
                <CommandPaletteItemRow
                  key={item.id}
                  item={item}
                  onSelect={handleSelectAction}
                />
              ))}
            </Command.Group>
          ))}
        </Command.List>
      </Command>
    </Command.Dialog>
  );
}

export default memo(CommandPaletteComponent);
