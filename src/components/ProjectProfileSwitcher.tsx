import { Layers, RefreshCw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { useProjectProfiles } from "../hooks/useProjectProfiles";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue } from "./ui/select";

export default function ProjectProfileSwitcher() {
  const locale = getLocale();
  const text = (zh: string, en: string, ja = en) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const { profiles, loading, pending, error, refresh, mutate } = useProjectProfiles();
  if (!profiles.length && !error) return null;
  const active = profiles.find((profile) => profile.isActive);

  return (
    <div className="project-profile-switcher">
      <Select
        value={active?.id ?? ""}
        disabled={!!pending || loading || !!error || !profiles.length}
        onValueChange={(profileId) => {
          const profile = profiles.find((item) => item.id === profileId);
          if (profile && !profile.isActive) void mutate("apply", { id: profile.id });
        }}
      >
        <SelectTrigger
          controlSize="sm"
          className="project-profile-trigger"
          title={text("切换项目配置档案", "Switch project profile", "プロジェクト設定を切り替え")}
          aria-label={text("切换项目配置档案", "Switch project profile", "プロジェクト設定を切り替え")}
        >
          <Layers size={14} className="shrink-0 text-muted-foreground" aria-hidden="true" />
          <SelectValue placeholder={text("未选择档案", "No profile", "未選択")} />
        </SelectTrigger>
        <SelectContent align="end" className="w-80 max-w-[calc(100vw-24px)]">
          <SelectGroup>
            <SelectLabel>{text("项目配置档案", "Project profiles", "プロジェクト設定")}</SelectLabel>
            {profiles.map((profile) => (
              <SelectItem key={profile.id} value={profile.id}>
                {profile.name}
              </SelectItem>
            ))}
          </SelectGroup>
        </SelectContent>
      </Select>
      <Button
        variant="ghost"
        size="icon-sm"
        title={
          error
            ? text("状态未能更新，点击重试", "Could not refresh; retry", "更新できません。再試行")
            : text("刷新项目档案", "Refresh project profiles", "プロジェクト設定を更新")
        }
        aria-label={text("刷新项目档案", "Refresh project profiles", "プロジェクト設定を更新")}
        onClick={() => void refresh()}
        disabled={!!pending || loading}
      >
        <RefreshCw
          size={14}
          className={loading ? "animate-spin" : error ? "text-[var(--warning)]" : ""}
          aria-hidden="true"
        />
      </Button>
    </div>
  );
}
