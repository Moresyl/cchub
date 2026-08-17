import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, ChevronDown, Layers, RefreshCw } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

type ProjectProfile = {
  id: string;
  name: string;
  description: string | null;
  isActive: boolean;
};

export default function ProjectProfileSwitcher() {
  const locale = getLocale();
  const text = (zh: string, en: string, ja = en) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const rootRef = useRef<HTMLDivElement>(null);
  const [profiles, setProfiles] = useState<ProjectProfile[]>([]);
  const [open, setOpen] = useState(false);
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

  useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [open]);

  const apply = useCallback(
    async (profile: ProjectProfile) => {
      if (profile.isActive) {
        setOpen(false);
        return;
      }
      setBusyId(profile.id);
      try {
        await invoke("apply_project_profile", { id: profile.id });
        setOpen(false);
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
    <div ref={rootRef} style={{ position: "relative" }}>
      <button
        className="theme-btn"
        type="button"
        title={text("切换项目配置档案", "Switch project profile", "プロジェクト設定を切り替え")}
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        style={{ display: "flex", alignItems: "center", gap: 5, maxWidth: 180 }}
      >
        <Layers size={15} />
        <span
          style={{ maxWidth: 118, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 11 }}
        >
          {active?.name ?? text("未选择档案", "No profile", "未選択")}
        </span>
        <ChevronDown size={12} />
      </button>
      {open && (
        <div
          className="section-card"
          style={{
            position: "absolute",
            top: "calc(100% + 8px)",
            right: 0,
            zIndex: 60,
            width: 260,
            padding: 6,
            boxShadow: "var(--shadow-lg)",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "7px 8px",
              color: "var(--text-muted)",
              fontSize: 11,
            }}
          >
            <span>{text("项目配置档案", "Project profiles", "プロジェクト設定")}</span>
            <button
              className="btn btn-ghost btn-icon-sm"
              type="button"
              title={text("刷新", "Refresh", "更新")}
              onClick={() => void load()}
            >
              <RefreshCw size={12} />
            </button>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
            {profiles.map((profile) => (
              <button
                key={profile.id}
                type="button"
                className="btn btn-ghost"
                onClick={() => void apply(profile)}
                disabled={busyId !== null}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  width: "100%",
                  justifyContent: "flex-start",
                  padding: "8px 9px",
                  textAlign: "left",
                }}
              >
                <Check
                  size={14}
                  style={{ color: profile.isActive ? "var(--success)" : "transparent", flexShrink: 0 }}
                />
                <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {profile.name}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
