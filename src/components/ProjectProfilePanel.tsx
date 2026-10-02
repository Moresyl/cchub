import { useEffect, useId, useRef, useState } from "react";
import { Check, Layers, Plus, RefreshCw, Save, Trash2, X } from "lucide-react";
import ConfirmDialog from "./ConfirmDialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { getLocale } from "../lib/i18n";
import { useProjectProfiles, type ProjectProfile } from "../hooks/useProjectProfiles";

function formatUpdatedAt(value: string, locale: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function hasControlCharacters(value: string, multiline = false) {
  return Array.from(value).some((character) => {
    const code = character.codePointAt(0)!;
    return (code < 32 || (code >= 127 && code <= 159)) && !(multiline && [9, 10, 13].includes(code));
  });
}

export default function ProjectProfilePanel() {
  const locale = getLocale();
  const text = (zh: string, en: string, ja = en) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const { profiles, loading, pending, error, committed, refresh, mutate } = useProjectProfiles();
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [pendingDelete, setPendingDelete] = useState<ProjectProfile | null>(null);
  const [pendingRefresh, setPendingRefresh] = useState<ProjectProfile | null>(null);
  const fieldId = useId();
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const locked = !!pending || loading || !!error;
  const nameValid = !!name.trim() && Array.from(name.trim()).length <= 120 && !hasControlCharacters(name);
  const descriptionValid = Array.from(description.trim()).length <= 2000 && !hasControlCharacters(description, true);

  async function create() {
    if (!nameValid || !descriptionValid) return;
    const saved = await mutate("create", { name: name.trim(), description: description.trim() || null });
    if (saved && mounted.current) {
      setName("");
      setDescription("");
      setShowCreate(false);
    }
  }

  async function replaceSnapshot() {
    if (!pendingRefresh) return;
    await mutate("update", {
      id: pendingRefresh.id,
      name: pendingRefresh.name,
      description: pendingRefresh.description,
      resnapshot: true,
    });
    if (mounted.current) setPendingRefresh(null);
  }

  async function remove() {
    if (!pendingDelete) return;
    await mutate("delete", { id: pendingDelete.id });
    if (mounted.current) setPendingDelete(null);
  }

  return (
    <section className="section-card mb-5 space-y-4" aria-labelledby={`${fieldId}-title`} aria-busy={!!pending}>
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex min-w-0 flex-1 basis-64 items-start gap-2">
          <Layers size={16} className="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />
          <div className="min-w-0">
            <h3 id={`${fieldId}-title`} className="text-sm font-semibold">
              {text("项目配置档案", "Project profiles", "プロジェクト設定")}
            </h3>
            <p className="page-subtitle mt-1">
              {text(
                "保存工作区和工具配置，一次切换整组配置。",
                "Save the workspace and tool profiles to switch them together.",
                "ワークスペースとツール設定をまとめて保存・切り替えます。",
              )}
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button
            variant="ghost"
            size="icon"
            aria-label={text("刷新项目档案", "Refresh project profiles", "プロジェクト設定を更新")}
            disabled={loading || !!pending}
            onClick={() => void refresh()}
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} aria-hidden="true" />
          </Button>
          <Button
            variant="secondary"
            disabled={!!pending}
            aria-expanded={showCreate}
            aria-controls={`${fieldId}-create`}
            onClick={() => setShowCreate((value) => !value)}
          >
            {showCreate ? <X size={14} aria-hidden="true" /> : <Plus size={14} aria-hidden="true" />}
            {showCreate ? text("取消", "Cancel", "キャンセル") : text("保存当前", "Save current", "現在を保存")}
          </Button>
        </div>
      </div>
      {showCreate && (
        <form
          id={`${fieldId}-create`}
          className="space-y-3 rounded-md border border-border bg-[var(--bg-card)] p-4"
          onSubmit={(event) => {
            event.preventDefault();
            void create();
          }}
        >
          <div className="space-y-1.5">
            <label htmlFor={`${fieldId}-name`} className="text-xs text-muted-foreground">
              {text("档案名称", "Profile name", "設定名")}
            </label>
            <Input
              id={`${fieldId}-name`}
              value={name}
              disabled={!!pending}
              onChange={(event) => setName(event.target.value)}
              autoFocus
              aria-invalid={name.length > 0 && !nameValid}
              aria-describedby={`${fieldId}-limits`}
            />
          </div>
          <div className="space-y-1.5">
            <label htmlFor={`${fieldId}-description`} className="text-xs text-muted-foreground">
              {text("说明（可选）", "Description (optional)", "説明（任意）")}
            </label>
            <Textarea
              id={`${fieldId}-description`}
              value={description}
              disabled={!!pending}
              onChange={(event) => setDescription(event.target.value)}
              aria-invalid={!descriptionValid}
              aria-describedby={`${fieldId}-limits`}
            />
          </div>
          <p id={`${fieldId}-limits`} className="text-xs text-muted-foreground">
            {text(
              "名称最多 120 字，说明最多 2000 字。",
              "Name: up to 120 characters. Description: up to 2,000.",
              "名前は120文字、説明は2,000文字まで。",
            )}
          </p>
          <div className="flex justify-end">
            <Button type="submit" disabled={locked || !nameValid || !descriptionValid}>
              <Save size={14} aria-hidden="true" />
              {pending === "create"
                ? text("保存中…", "Saving…", "保存中…")
                : text("保存档案", "Save profile", "設定を保存")}
            </Button>
          </div>
        </form>
      )}
      {error && (
        <div
          role="alert"
          className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border bg-[var(--bg-card)] p-3 text-xs"
        >
          <p className="min-w-0 flex-1 text-[var(--warning)]">
            {error === "refresh"
              ? text(
                  "操作已完成，但列表未能刷新。请刷新列表确认最新状态，不要重复提交。",
                  "The action completed, but the list could not refresh. Refresh to confirm the latest state; do not repeat the action.",
                  "操作は完了しましたが一覧を更新できません。再送せず一覧を更新してください。",
                )
              : error === "mutation"
                ? text(
                    "操作未完成。请刷新状态后重试，已输入的内容仍保留。",
                    "The action did not complete. Refresh before retrying; your draft is preserved.",
                    "操作を完了できませんでした。入力内容は保持されています。更新して再試行してください。",
                  )
                : text(
                    "未能读取项目档案，已加载的内容仍保留。请重试。",
                    "Could not read project profiles. Previously loaded profiles are preserved; retry.",
                    "読み込めませんでした。以前の一覧は保持されています。再試行してください。",
                  )}
          </p>
          <Button variant="secondary" disabled={loading || !!pending} onClick={() => void refresh()}>
            {text("刷新列表", "Refresh list", "一覧を更新")}
          </Button>
        </div>
      )}
      {committed && !error && (
        <p role="status" className="text-xs text-muted-foreground">
          {text("项目档案操作已完成。", "Project profile action completed.", "プロジェクト設定の操作が完了しました。")}
        </p>
      )}
      {loading && (
        <p role="status" className="text-xs text-muted-foreground">
          {text("加载中…", "Loading…", "読み込み中…")}
        </p>
      )}
      {!loading && !error && !profiles.length && (
        <p className="py-4 text-center text-xs text-muted-foreground">
          {text(
            "还没有项目档案，先保存当前配置。",
            "No project profiles yet. Save your current configuration to begin.",
            "現在の設定を保存して始めましょう。",
          )}
        </p>
      )}
      <div className="space-y-2">
        {profiles.map((profile) => (
          <article
            key={profile.id}
            aria-label={profile.name}
            className={`flex flex-wrap items-center gap-3 rounded-md border border-border p-3 ${profile.isActive ? "bg-[var(--bg-elevated)]" : "bg-[var(--bg-card)]"}`}
          >
            <div className="min-w-0 flex-1 basis-48 space-y-1">
              <div className="flex flex-wrap items-center gap-2">
                <h4 className="break-all text-sm font-medium">{profile.name}</h4>
                {profile.isActive && (
                  <span className="badge badge-success">
                    <Check size={11} aria-hidden="true" />
                    {text("当前", "Active", "現在")}
                  </span>
                )}
              </div>
              {profile.description && (
                <p className="whitespace-pre-line break-words text-xs text-muted-foreground">{profile.description}</p>
              )}
              <p className="text-xs text-muted-foreground">
                {profile.snapshot.configProfileIds.length} {text("个工具配置", "tool profiles", "ツール設定")} ·{" "}
                {formatUpdatedAt(profile.updatedAt, locale)}
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button
                variant="secondary"
                disabled={locked || profile.isActive}
                onClick={() => void mutate("apply", { id: profile.id })}
                aria-label={`${text("应用档案", "Apply profile", "設定を適用")} ${profile.name}`}
              >
                <Check size={14} aria-hidden="true" />
                {pending === profile.id ? text("处理中…", "Working…", "処理中…") : text("应用", "Apply", "適用")}
              </Button>
              <Button
                variant="ghost"
                size="icon"
                disabled={locked}
                onClick={() => setPendingRefresh(profile)}
                title={text("用当前状态更新快照", "Replace snapshot with current state", "現在の状態で更新")}
                aria-label={`${text("更新快照", "Update snapshot", "スナップショットを更新")} ${profile.name}`}
              >
                <RefreshCw size={14} aria-hidden="true" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                disabled={locked}
                onClick={() => setPendingDelete(profile)}
                title={text("删除档案", "Delete profile", "設定を削除")}
                aria-label={`${text("删除档案", "Delete profile", "設定を削除")} ${profile.name}`}
              >
                <Trash2 size={14} aria-hidden="true" />
              </Button>
            </div>
          </article>
        ))}
      </div>
      <ConfirmDialog
        isOpen={!!pendingRefresh}
        busy={!!pending}
        title={text("更新项目快照", "Update project snapshot", "スナップショットを更新")}
        message={text(
          `用当前工作区和工具配置替换「${pendingRefresh?.name}」的已保存快照？原快照将被替换。`,
          `Replace the saved snapshot of “${pendingRefresh?.name}” with the current workspace and tool profiles?`,
          `「${pendingRefresh?.name}」の保存済み設定を現在の状態に置き換えますか？`,
        )}
        confirmText={text("更新快照", "Update snapshot", "更新")}
        cancelText={text("取消", "Cancel", "キャンセル")}
        variant="info"
        onConfirm={() => void replaceSnapshot()}
        onCancel={() => setPendingRefresh(null)}
      />
      <ConfirmDialog
        isOpen={!!pendingDelete}
        busy={!!pending}
        title={text("删除项目档案", "Delete project profile", "プロジェクト設定を削除")}
        message={text(
          `确定删除「${pendingDelete?.name}」？只删除档案，保留工具配置和工作区。`,
          `Delete “${pendingDelete?.name}”? Tool profiles and workspaces will be preserved.`,
          `「${pendingDelete?.name}」を削除しますか？ツール設定とワークスペースは保持されます。`,
        )}
        confirmText={text("删除", "Delete", "削除")}
        cancelText={text("取消", "Cancel", "キャンセル")}
        variant="destructive"
        onConfirm={() => void remove()}
        onCancel={() => setPendingDelete(null)}
      />
    </section>
  );
}
