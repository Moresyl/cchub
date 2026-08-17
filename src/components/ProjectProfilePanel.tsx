import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Layers, Plus, RefreshCw, Save, Trash2, X } from "lucide-react";
import ConfirmDialog from "./ConfirmDialog";
import ErrorState from "./states/ErrorState";
import { getLocale } from "../lib/i18n";

type ProjectProfileSnapshot = {
  version: number;
  workspaceId: string | null;
  configProfileIds: string[];
};

type ProjectProfile = {
  id: string;
  name: string;
  description: string | null;
  snapshot: ProjectProfileSnapshot;
  updatedAt: string;
  lastAppliedAt: string | null;
  isActive: boolean;
};

function formatUpdatedAt(value: string, locale: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export default function ProjectProfilePanel() {
  const locale = getLocale();
  const text = (zh: string, en: string, ja = en) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const [profiles, setProfiles] = useState<ProjectProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<ProjectProfile | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await invoke<ProjectProfile[]>("get_project_profiles");
      setProfiles(next);
    } catch (cause) {
      setError(String(cause));
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

  const create = useCallback(async () => {
    if (!name.trim()) return;
    setBusyId("create");
    setError(null);
    try {
      await invoke("create_project_profile", {
        name: name.trim(),
        description: description.trim() || null,
      });
      setName("");
      setDescription("");
      setShowCreate(false);
      await load();
      window.dispatchEvent(new Event("cchub-project-profile-refresh"));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusyId(null);
    }
  }, [description, load, name]);

  const apply = useCallback(
    async (profile: ProjectProfile) => {
      setBusyId(profile.id);
      setError(null);
      try {
        await invoke("apply_project_profile", { id: profile.id });
        await load();
      } catch (cause) {
        setError(String(cause));
      } finally {
        setBusyId(null);
      }
    },
    [load],
  );

  const refresh = useCallback(
    async (profile: ProjectProfile) => {
      setBusyId(profile.id);
      setError(null);
      try {
        await invoke("update_project_profile", {
          id: profile.id,
          name: profile.name,
          description: profile.description,
          resnapshot: true,
        });
        await load();
        window.dispatchEvent(new Event("cchub-project-profile-refresh"));
      } catch (cause) {
        setError(String(cause));
      } finally {
        setBusyId(null);
      }
    },
    [load],
  );

  const remove = useCallback(
    async (profile: ProjectProfile) => {
      setBusyId(profile.id);
      setError(null);
      try {
        await invoke("delete_project_profile", { id: profile.id });
        setPendingDelete(null);
        await load();
        window.dispatchEvent(new Event("cchub-project-profile-refresh"));
      } catch (cause) {
        setError(String(cause));
      } finally {
        setBusyId(null);
      }
    },
    [load],
  );

  return (
    <section className="section-card" style={{ marginBottom: 20 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Layers size={16} style={{ color: "var(--accent)" }} />
          <div>
            <h3 style={{ fontSize: 14, fontWeight: 600 }}>
              {text("项目配置档案", "Project profiles", "プロジェクト設定")}
            </h3>
            <p className="page-subtitle" style={{ marginTop: 3 }}>
              {text(
                "保存当前工作区和各工具配置，可一键恢复。",
                "Save the active workspace and tool profiles for one-click restore.",
                "現在のワークスペースとツール設定を保存して復元します。",
              )}
            </p>
          </div>
        </div>
        <button
          className="btn btn-secondary btn-sm"
          onClick={() => setShowCreate((value) => !value)}
          title={text("从当前状态创建", "Create from current state", "現在の状態から作成")}
        >
          {showCreate ? <X size={14} /> : <Plus size={14} />}
          {showCreate ? text("取消", "Cancel", "キャンセル") : text("保存当前", "Save current", "現在を保存")}
        </button>
      </div>

      {showCreate && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8, margin: "14px 0" }}>
          <input
            className="input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={text("档案名称", "Profile name", "設定名")}
            autoFocus
          />
          <input
            className="input"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            placeholder={text("说明（可选）", "Description (optional)", "説明（任意）")}
          />
          <button
            className="btn btn-primary btn-sm"
            onClick={() => void create()}
            disabled={!name.trim() || busyId === "create"}
            style={{ alignSelf: "flex-end" }}
          >
            <Save size={14} />
            {text("保存档案", "Save profile", "設定を保存")}
          </button>
        </div>
      )}

      {error && (
        <ErrorState
          title={text("项目档案操作失败", "Project profile action failed", "プロジェクト設定の操作に失敗しました")}
          message={error}
          retryLabel={text("重试", "Retry", "再試行")}
          onRetry={() => void load()}
        />
      )}
      {loading ? (
        <div className="empty-state" style={{ padding: "18px 0" }}>
          {text("加载中...", "Loading...", "読み込み中...")}
        </div>
      ) : profiles.length === 0 ? (
        <div className="empty-state" style={{ padding: "18px 0" }}>
          {text("还没有项目档案", "No project profiles yet", "プロジェクト設定はまだありません")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 12 }}>
          {profiles.map((profile) => {
            const busy = busyId === profile.id;
            return (
              <div
                key={profile.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "10px 12px",
                  border: "1px solid var(--border-subtle)",
                  borderRadius: 6,
                  background: profile.isActive ? "var(--bg-elevated)" : "transparent",
                }}
              >
                <div style={{ minWidth: 0, flex: 1 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                    <strong
                      style={{ fontSize: 13, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                    >
                      {profile.name}
                    </strong>
                    {profile.isActive && (
                      <span className="badge badge-success">
                        <Check size={11} />
                        {text("当前", "Active", "現在")}
                      </span>
                    )}
                  </div>
                  <div className="page-subtitle" style={{ marginTop: 3 }}>
                    {profile.description || text("未填写说明", "No description", "説明なし")} ·{" "}
                    {profile.snapshot.configProfileIds.length} {text("个工具配置", "tool profiles", "ツール設定")} ·{" "}
                    {formatUpdatedAt(profile.updatedAt, locale)}
                  </div>
                </div>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => void apply(profile)}
                  disabled={busy}
                  title={text("应用档案", "Apply profile", "設定を適用")}
                >
                  <Check size={14} />
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => void refresh(profile)}
                  disabled={busy}
                  title={text("用当前状态更新快照", "Refresh snapshot from current state", "現在の状態で更新")}
                >
                  <RefreshCw size={14} />
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => setPendingDelete(profile)}
                  disabled={busy}
                  title={text("删除档案", "Delete profile", "設定を削除")}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            );
          })}
        </div>
      )}

      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={text("删除项目档案", "Delete project profile", "プロジェクト設定を削除")}
        message={text(
          `确定删除「${pendingDelete?.name}」？`,
          `Delete “${pendingDelete?.name}”?`,
          `「${pendingDelete?.name}」を削除しますか？`,
        )}
        confirmText={text("删除", "Delete", "削除")}
        variant="destructive"
        onConfirm={() => {
          if (pendingDelete) void remove(pendingDelete);
        }}
        onCancel={() => setPendingDelete(null)}
      />
    </section>
  );
}
