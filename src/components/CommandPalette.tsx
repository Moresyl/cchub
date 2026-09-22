import { memo, useEffect, useState } from "react";
import { Command } from "cmdk";
import { invoke } from "@tauri-apps/api/core";
import { ArrowRightLeft, Moon, Plus, Search, Sun } from "lucide-react";
import { useApplyConfigProfileMutation } from "../hooks/mutations";
import type { ConfigProfileQueryResult } from "../hooks/queries";
import { getLocale, t } from "../lib/i18n";
import { navigationSections } from "../lib/navigation";
import { getTheme, setTheme } from "../lib/theme";
import { Dialog, DialogContent, DialogTitle } from "./ui/dialog";
import { showToast } from "./Toast";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  navigate: (path: string) => void;
  currentPath: string;
}

function CommandPaletteComponent({ open, onOpenChange, navigate, currentPath }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [profiles, setProfiles] = useState<ConfigProfileQueryResult[]>([]);
  const applyProfile = useApplyConfigProfileMutation();
  const locale = getLocale();
  const i = t();
  const text = (zh: string, en: string, ja = en) => (locale === "zh" ? zh : locale === "ja" ? ja : en);

  useEffect(() => {
    if (!open) {
      setQuery("");
      return;
    }
    let active = true;
    invoke<ConfigProfileQueryResult[]>("get_config_profiles")
      .then((items) => {
        if (active) setProfiles(items);
      })
      .catch((error) => console.warn("Failed to load profiles for quick switch", error));
    return () => {
      active = false;
    };
  }, [open]);

  const close = () => onOpenChange(false);
  const goTo = (path: string) => {
    close();
    navigate(path);
  };
  const apply = async (profile: ConfigProfileQueryResult) => {
    close();
    try {
      await applyProfile.mutateAsync(profile.id);
      window.dispatchEvent(new Event("cchub-profiles-refresh"));
      void invoke("refresh_tray_provider_menu").catch((error) => {
        console.warn("Failed to refresh tray provider menu", error);
      });
      showToast("success", text(`已应用配置：${profile.name}`, `Profile applied: ${profile.name}`));
    } catch (error) {
      showToast("error", String(error));
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent hideClose className="command-palette">
        <DialogTitle className="sr-only">{text("快速切换", "Quick switch", "クイック切替")}</DialogTitle>
        <Command shouldFilter className="command-palette-body">
          <div className="command-palette-search">
            <Search size={16} aria-hidden="true" />
            <Command.Input
              autoFocus
              value={query}
              onValueChange={setQuery}
              placeholder={text("搜索配置或页面...", "Search profiles or pages...", "設定やページを検索...")}
            />
            <kbd>Esc</kbd>
          </div>
          <Command.List className="command-palette-list">
            <Command.Empty className="command-palette-empty">
              {text("没有匹配项", "No results", "該当する項目はありません")}
            </Command.Empty>
            <Command.Group heading={text("页面", "Pages", "ページ")}>
              {navigationSections
                .flatMap((section) => section.items)
                .map((item) => {
                  const Icon = item.icon;
                  return (
                    <Command.Item
                      key={item.path}
                      value={`${i.nav[item.labelKey]} ${item.path} ${item.labelKey}`}
                      onSelect={() => goTo(item.path)}
                    >
                      <Icon size={16} aria-hidden="true" />
                      <span>{i.nav[item.labelKey]}</span>
                      {currentPath === item.path && <span className="command-current">{text("当前", "Current")}</span>}
                    </Command.Item>
                  );
                })}
            </Command.Group>
            <Command.Group heading={text("配置", "Profiles", "設定")}>
              {profiles.map((profile) => (
                <Command.Item
                  key={profile.id}
                  value={`${profile.name} ${profile.tool_id} provider profile`}
                  onSelect={() => void apply(profile)}
                >
                  <ArrowRightLeft size={16} aria-hidden="true" />
                  <span>{profile.name}</span>
                  <span className="command-current">{profile.tool_id}</span>
                </Command.Item>
              ))}
            </Command.Group>
            <Command.Group heading={text("操作", "Actions", "操作")}>
              {currentPath === "/" && (
                <>
                  <Command.Item
                    onSelect={() => {
                      close();
                      window.dispatchEvent(new Event("cchub-shortcut-new"));
                    }}
                  >
                    <Plus size={16} aria-hidden="true" />
                    <span>{text("新增配置", "New profile", "設定を追加")}</span>
                  </Command.Item>
                  <Command.Item
                    onSelect={() => {
                      close();
                      window.dispatchEvent(new Event("cchub-shortcut-search"));
                    }}
                  >
                    <Search size={16} aria-hidden="true" />
                    <span>{text("搜索当前列表", "Search current list", "一覧を検索")}</span>
                  </Command.Item>
                </>
              )}
              <Command.Item
                onSelect={() => {
                  close();
                  setTheme(getTheme() === "dark" ? "light" : "dark");
                }}
              >
                {getTheme() === "dark" ? <Sun size={16} /> : <Moon size={16} />}
                <span>{text("切换主题", "Toggle theme", "テーマを切替")}</span>
              </Command.Item>
            </Command.Group>
          </Command.List>
        </Command>
      </DialogContent>
    </Dialog>
  );
}

export default memo(CommandPaletteComponent);
