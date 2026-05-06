import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { Command } from "cmdk";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  ArrowRightLeft,
  Bot,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  Languages,
  LayoutDashboard,
  Layers,
  Monitor,
  Moon,
  Plug,
  RefreshCw,
  Search,
  Settings,
  Shield,
  Store,
  Wrench,
  Webhook,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { checkAppUpdate } from "../lib/appUpdater";
import { t, getLocale, setLocale, type Locale } from "../lib/i18n";
import { getTheme, setTheme } from "../lib/theme";
import { useAutopilotFormStore } from "../stores/autopilotForm";
import { useCommandPaletteStore } from "../stores/commandPalette";
import type { ConfigProfileQueryResult } from "../hooks/queries";
import { showToast } from "./Toast";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  navigate: (path: string) => void;
  currentPath: string;
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
  | { type: "dispatch"; eventName: string }
  | {
      type: "effect";
      effect:
        | "toggle-theme"
        | "cycle-locale"
        | "check-update"
        | "open-autopilot-logs"
        | "start-autopilot"
        | "stop-autopilot";
    }
  | { type: "apply-profile"; id: string; name: string };

interface AutopilotStatusForPalette {
  logsRootDir: string;
}

interface CommandPaletteItemRowProps {
  item: PaletteItem;
  onSelect: (item: PaletteItem) => void;
}

const PAGE_ICONS: Record<string, LucideIcon> = {
  "/": LayoutDashboard,
  "/mcp-servers": Plug,
  "/mcp-clients": Monitor,
  "/logs": Activity,
  "/skills": Zap,
  "/workflows": GitBranch,
  "/autopilot": Bot,
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

function CommandPaletteItemRowComponent({ item, onSelect }: CommandPaletteItemRowProps) {
  const Icon = item.icon;
  const handleSelect = useCallback(() => {
    onSelect(item);
  }, [item, onSelect]);

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

function CommandPaletteComponent({ open, onOpenChange, navigate, currentPath }: CommandPaletteProps) {
  const [search, setSearch] = useState("");
  const [profiles, setProfiles] = useState<ConfigProfileQueryResult[]>([]);
  const autopilotForm = useAutopilotFormStore((state) => state.form);
  const recentCommandIds = useCommandPaletteStore((state) => state.recentCommandIds);
  const recordCommand = useCommandPaletteStore((state) => state.recordCommand);
  const i = t();
  const locale = getLocale();
  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  useEffect(() => {
    if (!open) {
      setSearch("");
      return;
    }
    invoke<ConfigProfileQueryResult[]>("get_config_profiles")
      .then((nextProfiles) => setProfiles(nextProfiles))
      .catch((error) => console.warn("Failed to load profiles for command palette", error));
  }, [open]);

  const pageGroupLabel = uiText("页面", "Pages", "ページ");
  const actionGroupLabel = uiText("操作", "Actions", "操作");
  const contextGroupLabel = uiText("当前页面", "Current Page", "現在のページ");
  const profileGroupLabel = uiText("应用配置档案", "Apply Profiles", "プロファイルを適用");
  const recentGroupLabel = uiText("最近使用", "Recent", "最近使用");
  const dialogLabel = uiText("全局命令面板", "Global Command Palette", "グローバルコマンドパレット");
  const searchPlaceholder = uiText("搜索页面或动作...", "Search pages or actions...", "ページや操作を検索...");
  const emptyLabel = uiText("没有匹配项", "No results found", "一致する項目がありません");

  const pageItems = useMemo<PaletteItem[]>(
    () => [
      {
        id: "page-dashboard",
        label: i.nav.dashboard,
        icon: PAGE_ICONS["/"],
        group: pageGroupLabel,
        keywords: ["dashboard", "home"],
        value: `${i.nav.dashboard} dashboard home`,
        action: { type: "navigate", path: "/" },
      },
      {
        id: "page-mcp-servers",
        label: i.nav.mcpServers,
        icon: PAGE_ICONS["/mcp-servers"],
        group: pageGroupLabel,
        keywords: ["mcp", "servers"],
        value: `${i.nav.mcpServers} mcp servers`,
        action: { type: "navigate", path: "/mcp-servers" },
      },
      {
        id: "page-mcp-clients",
        label: i.nav.mcpClients,
        icon: PAGE_ICONS["/mcp-clients"],
        group: pageGroupLabel,
        keywords: ["mcp", "clients"],
        value: `${i.nav.mcpClients} mcp clients`,
        action: { type: "navigate", path: "/mcp-clients" },
      },
      {
        id: "page-logs",
        label: i.nav.logs,
        icon: PAGE_ICONS["/logs"],
        group: pageGroupLabel,
        keywords: ["logs", "activity"],
        value: `${i.nav.logs} logs activity`,
        action: { type: "navigate", path: "/logs" },
      },
      {
        id: "page-skills",
        label: i.nav.skills,
        icon: PAGE_ICONS["/skills"],
        group: pageGroupLabel,
        keywords: ["skills", "plugins"],
        value: `${i.nav.skills} skills plugins`,
        action: { type: "navigate", path: "/skills" },
      },
      {
        id: "page-workflows",
        label: i.nav.workflows,
        icon: PAGE_ICONS["/workflows"],
        group: pageGroupLabel,
        keywords: ["workflows", "templates"],
        value: `${i.nav.workflows} workflows templates`,
        action: { type: "navigate", path: "/workflows" },
      },
      {
        id: "page-autopilot",
        label: i.nav.autopilot,
        icon: PAGE_ICONS["/autopilot"],
        group: pageGroupLabel,
        keywords: ["autopilot", "codex", "automation", "task"],
        value: `${i.nav.autopilot} autopilot codex automation task`,
        action: { type: "navigate", path: "/autopilot" },
      },
      {
        id: "page-marketplace",
        label: i.nav.marketplace,
        icon: PAGE_ICONS["/marketplace"],
        group: pageGroupLabel,
        keywords: ["market", "plugins"],
        value: `${i.nav.marketplace} market plugins`,
        action: { type: "navigate", path: "/marketplace" },
      },
      {
        id: "page-hooks",
        label: i.nav.hooks,
        icon: PAGE_ICONS["/hooks"],
        group: pageGroupLabel,
        keywords: ["hooks"],
        value: `${i.nav.hooks} hooks`,
        action: { type: "navigate", path: "/hooks" },
      },
      {
        id: "page-workspaces",
        label: i.nav.workspaces,
        icon: PAGE_ICONS["/workspaces"],
        group: pageGroupLabel,
        keywords: ["workspaces"],
        value: `${i.nav.workspaces} workspaces`,
        action: { type: "navigate", path: "/workspaces" },
      },
      {
        id: "page-profiles",
        label: i.nav.profiles,
        icon: PAGE_ICONS["/profiles"],
        group: pageGroupLabel,
        keywords: ["profiles", "providers"],
        value: `${i.nav.profiles} profiles providers`,
        action: { type: "navigate", path: "/profiles" },
      },
      {
        id: "page-sessions",
        label: i.nav.sessions,
        icon: PAGE_ICONS["/sessions"],
        group: pageGroupLabel,
        keywords: ["sessions", "history"],
        value: `${i.nav.sessions} sessions history`,
        action: { type: "navigate", path: "/sessions" },
      },
      {
        id: "page-claude-md",
        label: i.nav.claudeMd,
        icon: PAGE_ICONS["/claude-md"],
        group: pageGroupLabel,
        keywords: ["claude", "docs"],
        value: `${i.nav.claudeMd} claude docs`,
        action: { type: "navigate", path: "/claude-md" },
      },
      {
        id: "page-config-files",
        label: i.nav.configFiles,
        icon: PAGE_ICONS["/config-files"],
        group: pageGroupLabel,
        keywords: ["config", "files"],
        value: `${i.nav.configFiles} config files`,
        action: { type: "navigate", path: "/config-files" },
      },
      {
        id: "page-tools",
        label: i.nav.tools,
        icon: PAGE_ICONS["/tools"],
        group: pageGroupLabel,
        keywords: ["tools"],
        value: `${i.nav.tools} tools`,
        action: { type: "navigate", path: "/tools" },
      },
      {
        id: "page-security",
        label: i.nav.security,
        icon: PAGE_ICONS["/security"],
        group: pageGroupLabel,
        keywords: ["security", "audit"],
        value: `${i.nav.security} security audit`,
        action: { type: "navigate", path: "/security" },
      },
      {
        id: "page-settings",
        label: i.nav.settings,
        icon: PAGE_ICONS["/settings"],
        group: pageGroupLabel,
        keywords: ["settings", "preferences"],
        value: `${i.nav.settings} settings preferences`,
        action: { type: "navigate", path: "/settings" },
      },
    ],
    [i.nav, pageGroupLabel],
  );

  const contextItems = useMemo<PaletteItem[]>(() => {
    const supportsSave = new Set([
      "/claude-md",
      "/config-files",
      "/hooks",
      "/mcp-servers",
      "/mcp-clients",
      "/profiles",
      "/workflows",
      "/workspaces",
    ]).has(currentPath);
    const supportsNew = new Set([
      "/claude-md",
      "/hooks",
      "/mcp-servers",
      "/mcp-clients",
      "/profiles",
      "/workflows",
      "/workspaces",
    ]).has(currentPath);
    const supportsSearch = new Set(["/marketplace", "/profiles", "/sessions", "/skills"]).has(currentPath);
    const items: PaletteItem[] = [];

    if (supportsSave) {
      items.push({
        id: "action-save",
        label: uiText("保存当前内容", "Save Current Content", "現在の内容を保存"),
        icon: FileText,
        group: contextGroupLabel,
        keywords: ["save", "write"],
        value: `${uiText("保存当前内容", "Save Current Content", "現在の内容を保存")} save write`,
        action: { type: "dispatch", eventName: "cchub-shortcut-save" },
      });
    }
    if (supportsNew) {
      items.push({
        id: "action-new",
        label: uiText("新建当前页面项", "Create New Item", "新規項目を作成"),
        icon: Layers,
        group: contextGroupLabel,
        keywords: ["new", "create", "add"],
        value: `${uiText("新建当前页面项", "Create New Item", "新規項目を作成")} new create add`,
        action: { type: "dispatch", eventName: "cchub-shortcut-new" },
      });
    }
    if (supportsSearch) {
      items.push({
        id: "action-search",
        label: uiText("聚焦页面搜索", "Focus Page Search", "ページ検索にフォーカス"),
        icon: Search,
        group: contextGroupLabel,
        keywords: ["search", "find", "filter"],
        value: `${uiText("聚焦页面搜索", "Focus Page Search", "ページ検索にフォーカス")} search find filter`,
        action: { type: "dispatch", eventName: "cchub-shortcut-search" },
      });
    }

    return items;
  }, [contextGroupLabel, currentPath, uiText]);

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
        id: "action-settings",
        label: uiText("打开设置", "Open Settings", "設定を開く"),
        icon: Settings,
        group: actionGroupLabel,
        keywords: ["settings", "preferences"],
        value: `${uiText("打开设置", "Open Settings", "設定を開く")} settings preferences`,
        action: { type: "navigate", path: "/settings" },
      },
      {
        id: "action-toggle-theme",
        label: uiText("切换主题", "Toggle Theme", "テーマを切り替え"),
        icon: Moon,
        group: actionGroupLabel,
        keywords: ["theme", "dark", "light"],
        value: `${uiText("切换主题", "Toggle Theme", "テーマを切り替え")} theme dark light`,
        action: { type: "effect", effect: "toggle-theme" },
      },
      {
        id: "action-cycle-locale",
        label: uiText("切换语言", "Switch Language", "言語を切り替え"),
        icon: Languages,
        group: actionGroupLabel,
        keywords: ["language", "locale", "zh", "en", "ja"],
        value: `${uiText("切换语言", "Switch Language", "言語を切り替え")} language locale zh en ja`,
        action: { type: "effect", effect: "cycle-locale" },
      },
      {
        id: "action-check-update",
        label: uiText("检查更新", "Check for Updates", "更新を確認"),
        icon: RefreshCw,
        group: actionGroupLabel,
        keywords: ["update", "version", "release"],
        value: `${uiText("检查更新", "Check for Updates", "更新を確認")} update version release`,
        action: { type: "effect", effect: "check-update" },
      },
      {
        id: "action-open-autopilot-logs",
        label: uiText("打开 Autopilot 日志目录", "Open Autopilot Logs Folder", "Autopilot ログフォルダを開く"),
        icon: FolderOpen,
        group: actionGroupLabel,
        keywords: ["autopilot", "logs", "folder"],
        value: `${uiText("打开 Autopilot 日志目录", "Open Autopilot Logs Folder", "Autopilot ログフォルダを開く")} autopilot logs folder`,
        action: { type: "effect", effect: "open-autopilot-logs" },
      },
      {
        id: "action-start-autopilot",
        label: uiText("启动 Autopilot", "Start Autopilot", "Autopilot を開始"),
        icon: Bot,
        group: actionGroupLabel,
        keywords: ["autopilot", "start", "run"],
        value: `${uiText("启动 Autopilot", "Start Autopilot", "Autopilot を開始")} autopilot start run`,
        action: { type: "effect", effect: "start-autopilot" },
      },
      {
        id: "action-stop-autopilot",
        label: uiText("停止 Autopilot", "Stop Autopilot", "Autopilot を停止"),
        icon: Bot,
        group: actionGroupLabel,
        keywords: ["autopilot", "stop"],
        value: `${uiText("停止 Autopilot", "Stop Autopilot", "Autopilot を停止")} autopilot stop`,
        action: { type: "effect", effect: "stop-autopilot" },
      },
    ],
    [actionGroupLabel, uiText],
  );

  const profileItems = useMemo<PaletteItem[]>(
    () =>
      profiles.slice(0, 12).map((profile) => ({
        id: `profile-${profile.id}`,
        label: `${profile.name} · ${profile.tool_id}`,
        icon: ArrowRightLeft,
        group: profileGroupLabel,
        keywords: ["profile", "apply", profile.name, profile.tool_id],
        value: `${uiText("应用配置档案", "Apply Profile", "プロファイルを適用")} ${profile.name} ${profile.tool_id}`,
        action: { type: "apply-profile", id: profile.id, name: profile.name },
      })),
    [profileGroupLabel, profiles, uiText],
  );

  const groupedItems = useMemo(() => {
    const all = [...contextItems, ...profileItems, ...pageItems, ...actionItems];
    const groups = new Map<string, PaletteItem[]>();
    const recentItems = recentCommandIds
      .map((id) => all.find((item) => item.id === id))
      .filter((item): item is PaletteItem => Boolean(item));

    if (recentItems.length > 0) {
      groups.set(recentGroupLabel, recentItems);
    }

    for (const item of all) {
      if (!groups.has(item.group)) {
        groups.set(item.group, []);
      }
      groups.get(item.group)?.push(item);
    }

    return Array.from(groups.entries());
  }, [actionItems, contextItems, pageItems, profileItems, recentCommandIds, recentGroupLabel]);

  const runEffectAction = useCallback(
    async (effect: Extract<PaletteAction, { type: "effect" }>["effect"]) => {
      try {
        if (effect === "toggle-theme") {
          const nextTheme = getTheme() === "dark" ? "light" : "dark";
          setTheme(nextTheme);
          showToast("success", uiText("主题已切换", "Theme switched", "テーマを切り替えました"));
          return;
        }

        if (effect === "cycle-locale") {
          const order: Locale[] = ["zh", "en", "ja"];
          const currentIndex = order.indexOf(getLocale());
          const nextLocale = order[(currentIndex + 1) % order.length];
          setLocale(nextLocale);
          showToast(
            "success",
            uiText(
              "语言已切换，正在刷新界面",
              "Language switched, refreshing UI",
              "言語を切り替えました。画面を更新します",
            ),
          );
          window.setTimeout(() => window.location.reload(), 250);
          return;
        }

        if (effect === "check-update") {
          const { result } = await checkAppUpdate();
          if (result.update_available && result.latest_version) {
            showToast(
              "info",
              uiText(
                `发现新版本 v${result.latest_version}`,
                `Update available: v${result.latest_version}`,
                `新しいバージョン v${result.latest_version} があります`,
              ),
              8000,
            );
          } else {
            showToast("success", uiText("当前已是最新版本", "You are up to date", "最新版です"));
          }
          return;
        }

        if (effect === "open-autopilot-logs") {
          const status = await invoke<AutopilotStatusForPalette>("get_autopilot_status");
          if (!status.logsRootDir) {
            showToast(
              "error",
              uiText(
                "Autopilot 日志目录尚未创建",
                "Autopilot log folder is not ready",
                "Autopilot ログフォルダはまだありません",
              ),
            );
            return;
          }
          await invoke("open_in_system", { target: status.logsRootDir });
          return;
        }

        if (effect === "start-autopilot") {
          if (autopilotForm.taskFiles.length === 0) {
            showToast(
              "error",
              uiText(
                "请先选择 Autopilot 任务文件",
                "Choose an Autopilot task file first",
                "先に Autopilot タスクファイルを選択してください",
              ),
            );
            navigate("/autopilot");
            return;
          }
          if (autopilotForm.bypass) {
            showToast(
              "info",
              uiText(
                "Bypass 模式需要在 Autopilot 页面确认",
                "Bypass mode requires confirmation on the Autopilot page",
                "Bypass モードは Autopilot ページで確認が必要です",
              ),
            );
            navigate("/autopilot");
            return;
          }
          await invoke("start_autopilot", {
            request: {
              taskFile: autopilotForm.taskFiles[0] ?? "",
              taskFiles: autopilotForm.taskFiles,
              workdir: autopilotForm.workdir,
              model: autopilotForm.model,
              profile: autopilotForm.profile,
              interval: Number(autopilotForm.interval || "0"),
              maxAttempts: Number(autopilotForm.maxAttempts || "0"),
              fresh: autopilotForm.fresh,
              dryRun: autopilotForm.dryRun,
              skipGitCheck: autopilotForm.skipGitCheck,
              bypass: autopilotForm.bypass,
              fullAuto: autopilotForm.fullAuto,
              verbose: autopilotForm.verbose,
            },
          });
          showToast("success", uiText("Autopilot 已启动", "Autopilot started", "Autopilot を起動しました"));
          return;
        }

        if (effect === "stop-autopilot") {
          await invoke("stop_autopilot");
          showToast("success", uiText("停止请求已发送", "Stop request sent", "停止要求を送信しました"));
        }
      } catch (error) {
        showToast("error", String(error));
      }
    },
    [autopilotForm, navigate, uiText],
  );

  const handleSelectItem = useCallback(
    (item: PaletteItem) => {
      recordCommand(item.id);
      const { action } = item;
      onOpenChange(false);

      if (action.type === "navigate") {
        navigate(action.path);
        return;
      }

      if (action.type === "reload") {
        window.location.reload();
        return;
      }

      if (action.type === "effect") {
        void runEffectAction(action.effect);
        return;
      }

      if (action.type === "apply-profile") {
        void invoke("apply_config_profile", { id: action.id })
          .then(() =>
            invoke("refresh_tray_provider_menu").catch((error) => {
              console.warn("Failed to refresh tray after applying profile from command palette", error);
            }),
          )
          .then(() => {
            showToast(
              "success",
              uiText(
                `已应用配置档案: ${action.name}`,
                `Profile applied: ${action.name}`,
                `プロファイルを適用しました: ${action.name}`,
              ),
            );
          })
          .catch((error) => showToast("error", String(error)));
        return;
      }

      window.dispatchEvent(new CustomEvent(action.eventName));
    },
    [navigate, onOpenChange, recordCommand, runEffectAction, uiText],
  );

  return (
    <Command.Dialog open={open} onOpenChange={onOpenChange} label={dialogLabel}>
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
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            padding: "14px 16px",
            borderBottom: "1px solid var(--border-default)",
          }}
        >
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
          <span className="badge badge-muted" style={{ fontSize: 10, flexShrink: 0 }}>
            Ctrl+K
          </span>
        </div>
        <Command.List style={{ maxHeight: 420, overflowY: "auto", padding: 8 }}>
          <Command.Empty style={{ padding: 20, fontSize: 13, color: "var(--text-muted)", textAlign: "center" }}>
            {emptyLabel}
          </Command.Empty>
          {groupedItems.map(([group, items]) => (
            <Command.Group key={group} heading={group}>
              {items.map((item) => (
                <CommandPaletteItemRow key={item.id} item={item} onSelect={handleSelectItem} />
              ))}
            </Command.Group>
          ))}
        </Command.List>
      </Command>
    </Command.Dialog>
  );
}

export default memo(CommandPaletteComponent);
