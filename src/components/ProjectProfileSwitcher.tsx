import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Layers, RefreshCw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectItem, SelectLabel, SelectTrigger, SelectValue } from "./ui/select";

type ProjectProfile = {
  id: string;
  name: string;
  description: string | null;
  isActive: boolean;
};

export default function ProjectProfileSwitcher() {
  const locale = getLocale();
  const text = (zh: string, en: string, ja = en) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const [profiles, setProfiles] = useState<ProjectProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setProfiles(await invoke<ProjectProfile[]>("get_project_profiles"));
    } catch {
      setProfiles([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const refreshListener = () => void load();
    window.addEventListener("cchub-project-profile-refresh", refreshListener);
    return () => window.removeEventListener("cchub-project-profile-refresh", refreshListener);
  }, [load]);

  const apply = useCallback(
    async (profile: ProjectProfile) => {
      if (profile.isActive) return;
      setBusyId(profile.id);
      try {
        await invoke("apply_project_profile", { id: profile.id });
        window.dispatchEvent(new Event("cchub-project-profile-refresh"));
        await load();
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setBusyId(null);
      }
    },
    [load],
  );

  if (loading || profiles.length === 0) return null;
  const active = profiles.find((profile) => profile.isActive);

  return (
    <div className="flex items-center gap-1.5">
      <Select
        value={active?.id}
        disabled={busyId !== null}
        onValueChange={(profileId) => {
          const profile = profiles.find((item) => item.id === profileId);
          if (profile) void apply(profile);
        }}
      >
        <SelectTrigger
          className="w-[180px]"
          title={text("切换项目配置档案", "Switch project profile", "プロジェクト設定を切り替え")}
        >
          <Layers size={14} className="shrink-0 text-muted-foreground" aria-hidden="true" />
          <SelectValue placeholder={text("未选择档案", "No profile", "未選択")} />
        </SelectTrigger>
        <SelectContent align="end">
          <SelectLabel>{text("项目配置档案", "Project profiles", "プロジェクト設定")}</SelectLabel>
          {profiles.map((profile) => (
            <SelectItem key={profile.id} value={profile.id}>
              {profile.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button
        variant="ghost"
        size="icon"
        title={text("刷新项目档案", "Refresh project profiles", "プロジェクト設定を更新")}
        aria-label={text("刷新项目档案", "Refresh project profiles", "プロジェクト設定を更新")}
        onClick={() => void load()}
        disabled={busyId !== null}
      >
        <RefreshCw size={13} aria-hidden="true" />
      </Button>
    </div>
  );
}
