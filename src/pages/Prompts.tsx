import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Download, FileText, Pencil, Plus, RefreshCw, Search, Trash2, X } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import EmptyState from "../components/states/EmptyState";
import ErrorState from "../components/states/ErrorState";
import LoadingState from "../components/states/LoadingState";
import MarkdownEditor from "../components/MarkdownEditor";
import { showToast } from "../components/Toast";
import { fetchVisibleApps, type ManagedAppId } from "../lib/appPreferences";
import { getLocale } from "../lib/i18n";

type PromptApp = "claude" | "codex" | "gemini" | "opencode" | "openclaw" | "hermes" | "pi";

interface PromptRecord {
  id: string;
  name: string;
  content: string;
  description?: string | null;
  enabled: boolean;
  createdAt: number;
  updatedAt: number;
}

interface PromptDraft {
  id: string;
  name: string;
  description: string;
  content: string;
  enabled: boolean;
}

const APP_OPTIONS: Array<{ id: PromptApp; label: string; file: string }> = [
  { id: "claude", label: "Claude", file: "~/.claude/CLAUDE.md" },
  { id: "codex", label: "Codex", file: "~/.codex/AGENTS.md" },
  { id: "gemini", label: "Gemini", file: "~/.gemini/GEMINI.md" },
  { id: "opencode", label: "OpenCode", file: "~/.config/opencode/AGENTS.md" },
  { id: "openclaw", label: "OpenClaw", file: "~/.openclaw/AGENTS.md" },
  { id: "hermes", label: "Hermes", file: "~/.hermes/SOUL.md" },
  { id: "pi", label: "Pi", file: "~/.pi/agent/AGENTS.md" },
];

function createId() {
  return globalThis.crypto?.randomUUID?.() ?? `prompt-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function emptyDraft(): PromptDraft {
  return { id: createId(), name: "", description: "", content: "", enabled: false };
}

export default function Prompts() {
  const locale = getLocale();
  const uiText = useCallback(
    (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en),
    [locale],
  );
  const [activeApp, setActiveApp] = useState<PromptApp>("claude");
  const [visibleApps, setVisibleApps] = useState<PromptApp[]>(APP_OPTIONS.map((option) => option.id));
  const [prompts, setPrompts] = useState<Record<string, PromptRecord>>({});
  const [liveContent, setLiveContent] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [writing, setWriting] = useState(false);
  const [draft, setDraft] = useState<PromptDraft | null>(null);
  const [pendingDelete, setPendingDelete] = useState<PromptRecord | null>(null);
  const requestSequence = useRef(0);

  const load = useCallback(async () => {
    const sequence = ++requestSequence.current;
    setLoading(true);
    setLoadError(null);
    try {
      const [nextPrompts, nextLiveContent] = await Promise.all([
        invoke<Record<string, PromptRecord>>("get_prompts", { app: activeApp }),
        invoke<string | null>("get_current_prompt_file_content", { app: activeApp }).catch(() => null),
      ]);
      if (requestSequence.current !== sequence) return;
      setPrompts(nextPrompts);
      setLiveContent(nextLiveContent);
    } catch (error) {
      if (requestSequence.current === sequence) setLoadError(String(error));
    } finally {
      if (requestSequence.current === sequence) setLoading(false);
    }
  }, [activeApp]);

  useEffect(() => {
    fetchVisibleApps()
      .then((apps) => {
        const supported = APP_OPTIONS.map((option) => option.id).filter((app) => apps.includes(app as ManagedAppId));
        setVisibleApps(supported.length > 0 ? supported : APP_OPTIONS.map((option) => option.id));
        if (supported.length > 0) {
          setActiveApp((current) => (supported.includes(current) ? current : supported[0]));
        }
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    setPrompts({});
    setLiveContent(null);
    setDraft(null);
    setPendingDelete(null);
    setSearch("");
    void load();
  }, [load]);

  const orderedPrompts = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return Object.values(prompts)
      .filter((prompt) => {
        if (!query) return true;
        return `${prompt.name}\n${prompt.description ?? ""}\n${prompt.content}`.toLocaleLowerCase().includes(query);
      })
      .sort((left, right) => Number(right.enabled) - Number(left.enabled) || right.updatedAt - left.updatedAt);
  }, [prompts, search]);

  const appOption = APP_OPTIONS.find((option) => option.id === activeApp) ?? APP_OPTIONS[0];
  const activePrompt = Object.values(prompts).find((prompt) => prompt.enabled);

  const saveDraft = useCallback(
    async (activate: boolean) => {
      if (!draft) return;
      if (!draft.name.trim()) {
        showToast("error", uiText("请输入名称", "Enter a name", "名前を入力してください"));
        return;
      }
      setWriting(true);
      try {
        await invoke("upsert_prompt", {
          app: activeApp,
          id: draft.id,
          prompt: {
            name: draft.name.trim(),
            description: draft.description.trim() || null,
            content: draft.content,
            enabled: activate || draft.enabled,
          },
        });
        showToast(
          "success",
          activate
            ? uiText("已保存并写入工具配置", "Saved and activated", "保存して有効化しました")
            : uiText("已保存", "Saved", "保存しました"),
        );
        setDraft(null);
        await load();
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setWriting(false);
      }
    },
    [activeApp, draft, load, uiText],
  );

  const activatePrompt = useCallback(
    async (prompt: PromptRecord) => {
      setWriting(true);
      try {
        await invoke("enable_prompt", { app: activeApp, id: prompt.id });
        showToast("success", uiText("已写入当前工具", "Activated for this tool", "このツールで有効化しました"));
        await load();
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setWriting(false);
      }
    },
    [activeApp, load, uiText],
  );

  const deletePrompt = useCallback(async () => {
    if (!pendingDelete) return;
    setWriting(true);
    try {
      await invoke("delete_prompt", { app: activeApp, id: pendingDelete.id });
      showToast("success", uiText("已删除", "Deleted", "削除しました"));
      setPendingDelete(null);
      await load();
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setWriting(false);
    }
  }, [activeApp, load, pendingDelete, uiText]);

  const importFromFile = useCallback(async () => {
    setWriting(true);
    try {
      await invoke<string>("import_prompt_from_file", { app: activeApp });
      showToast(
        "success",
        uiText(
          "已从当前指令文件导入",
          "Imported from the live instruction file",
          "現在の指示ファイルからインポートしました",
        ),
      );
      await load();
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setWriting(false);
    }
  }, [activeApp, load, uiText]);

  useEffect(() => {
    const handleNew = () => setDraft(emptyDraft());
    const handleSave = () => {
      if (draft && !writing) void saveDraft(false);
    };
    const handleEscape = () => {
      if (search.trim()) {
        setSearch("");
        return;
      }
      setDraft(null);
    };
    window.addEventListener("cchub-shortcut-new", handleNew);
    window.addEventListener("cchub-shortcut-save", handleSave);
    window.addEventListener("cchub-shortcut-escape", handleEscape);
    return () => {
      window.removeEventListener("cchub-shortcut-new", handleNew);
      window.removeEventListener("cchub-shortcut-save", handleSave);
      window.removeEventListener("cchub-shortcut-escape", handleEscape);
    };
  }, [draft, saveDraft, search, writing]);

  if (draft) {
    return (
      <div className="page-stack">
        <div className="page-header">
          <div>
            <div className="page-title-row">
              <FileText size={19} />
              <h1 className="page-title">{draft.name || uiText("新建 Prompt", "New Prompt", "新規 Prompt")}</h1>
            </div>
            <p className="page-subtitle">
              {appOption.label} · {appOption.file}
            </p>
          </div>
          <button className="btn btn-ghost btn-sm" type="button" onClick={() => setDraft(null)} disabled={writing}>
            <X size={14} />
            {uiText("关闭", "Close", "閉じる")}
          </button>
        </div>
        <div className="section-card" style={{ display: "grid", gap: 14 }}>
          <label style={{ display: "grid", gap: 6, fontSize: 12, fontWeight: 600 }}>
            {uiText("名称", "Name", "名前")}
            <input
              className="input"
              maxLength={120}
              value={draft.name}
              onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              autoFocus
            />
          </label>
          <label style={{ display: "grid", gap: 6, fontSize: 12, fontWeight: 600 }}>
            {uiText("说明（可选）", "Description (optional)", "説明（任意）")}
            <input
              className="input"
              maxLength={2000}
              value={draft.description}
              onChange={(event) => setDraft({ ...draft, description: event.target.value })}
            />
          </label>
          <div style={{ display: "grid", gap: 6 }}>
            <span style={{ fontSize: 12, fontWeight: 600 }}>{uiText("内容", "Content", "内容")}</span>
            <MarkdownEditor
              value={draft.content}
              onChange={(content) => setDraft({ ...draft, content })}
              minHeight={360}
            />
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, flexWrap: "wrap" }}>
            <button
              className="btn btn-secondary btn-sm"
              type="button"
              onClick={() => setDraft(null)}
              disabled={writing}
            >
              {uiText("取消", "Cancel", "キャンセル")}
            </button>
            <button
              className="btn btn-secondary btn-sm"
              type="button"
              onClick={() => void saveDraft(false)}
              disabled={writing}
            >
              <Check size={14} />
              {uiText("保存", "Save", "保存")}
            </button>
            <button
              className="btn btn-primary btn-sm"
              type="button"
              onClick={() => void saveDraft(true)}
              disabled={writing}
            >
              <FileText size={14} />
              {uiText("保存并启用", "Save & activate", "保存して有効化")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (loading && Object.keys(prompts).length === 0)
    return <LoadingState label={uiText("正在加载 Prompt...", "Loading prompts...", "Prompt を読み込み中...")} />;
  if (loadError && Object.keys(prompts).length === 0)
    return (
      <ErrorState
        title={uiText("Prompt 加载失败", "Failed to load prompts", "Prompt の読み込みに失敗しました")}
        message={loadError}
        retryLabel={uiText("重试", "Retry", "再試行")}
        onRetry={() => void load()}
      />
    );

  return (
    <div className="page-stack">
      <div className="page-header">
        <div>
          <div className="page-title-row">
            <FileText size={19} />
            <h1 className="page-title">{uiText("Prompt 库", "Prompt Library", "Prompt ライブラリ")}</h1>
          </div>
          <p className="page-subtitle">
            {uiText(
              "按工具隔离管理指令版本，启用时原子写入对应配置文件。",
              "Manage instruction versions per tool and atomically write the active version.",
              "ツールごとに指示のバージョンを管理し、有効化時に安全に書き込みます。",
            )}
          </p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={() => void load()}
            disabled={loading || writing}
            title={uiText("刷新", "Refresh", "更新")}
          >
            <RefreshCw size={14} className={loading ? "spin" : undefined} />
          </button>
          <button
            className="btn btn-secondary btn-sm"
            type="button"
            onClick={() => void importFromFile()}
            disabled={writing}
          >
            <Download size={14} />
            {uiText("导入当前文件", "Import live file", "現在のファイルをインポート")}
          </button>
          <button
            className="btn btn-primary btn-sm"
            type="button"
            onClick={() => setDraft(emptyDraft())}
            disabled={writing}
          >
            <Plus size={14} />
            {uiText("新建", "New", "新規")}
          </button>
        </div>
      </div>

      <div className="tab-bar" style={{ flexWrap: "wrap" }}>
        {APP_OPTIONS.filter((option) => visibleApps.includes(option.id)).map((option) => (
          <button
            key={option.id}
            className={`tab-item ${activeApp === option.id ? "active" : ""}`}
            type="button"
            onClick={() => setActiveApp(option.id)}
          >
            {option.label}
          </button>
        ))}
      </div>

      <div
        className="section-card"
        style={{ display: "grid", gridTemplateColumns: "minmax(180px, 1fr) minmax(180px, 1fr)", gap: 14 }}
      >
        <div>
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {uiText("当前文件", "Live file", "現在のファイル")}
          </div>
          <div style={{ fontSize: 12, marginTop: 5, wordBreak: "break-all" }}>{appOption.file}</div>
        </div>
        <div>
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {uiText("启用版本", "Active version", "有効なバージョン")}
          </div>
          <div style={{ fontSize: 12, marginTop: 5 }}>
            {activePrompt?.name ??
              (liveContent !== null
                ? uiText(
                    "文件存在，尚未纳入版本库",
                    "Live file exists but is not versioned",
                    "ファイルは存在しますが未登録です",
                  )
                : uiText("未启用", "Not active", "未有効"))}
          </div>
        </div>
      </div>

      {loadError ? <div className="inline-error">{loadError}</div> : null}
      <div style={{ position: "relative", maxWidth: 420 }}>
        <Search
          size={14}
          style={{
            position: "absolute",
            left: 11,
            top: "50%",
            transform: "translateY(-50%)",
            color: "var(--text-muted)",
          }}
        />
        <input
          className="input"
          style={{ width: "100%", paddingLeft: 32 }}
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={uiText("搜索名称、说明或内容", "Search name, description, or content", "名前・説明・内容を検索")}
        />
      </div>

      {orderedPrompts.length === 0 ? (
        <EmptyState
          title={uiText("暂无 Prompt", "No prompts", "Prompt はありません")}
          description={uiText(
            "新建一个版本，或从当前工具的指令文件导入。",
            "Create a version or import the tool's live instruction file.",
            "新規作成するか、現在の指示ファイルからインポートしてください。",
          )}
          action={
            <button className="btn btn-primary btn-sm" type="button" onClick={() => setDraft(emptyDraft())}>
              <Plus size={14} />
              {uiText("新建 Prompt", "New prompt", "Prompt を作成")}
            </button>
          }
        />
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: 12 }}>
          {orderedPrompts.map((prompt) => (
            <div
              key={prompt.id}
              className="section-card"
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 10,
                borderColor: prompt.enabled ? "var(--accent)" : undefined,
              }}
            >
              <div style={{ display: "flex", justifyContent: "space-between", gap: 10, alignItems: "flex-start" }}>
                <div style={{ minWidth: 0 }}>
                  <div style={{ display: "flex", gap: 7, alignItems: "center", flexWrap: "wrap" }}>
                    <strong style={{ fontSize: 14 }}>{prompt.name}</strong>
                    {prompt.enabled ? (
                      <span className="badge badge-success">{uiText("已启用", "Active", "有効")}</span>
                    ) : null}
                  </div>
                  {prompt.description ? (
                    <div style={{ marginTop: 5, color: "var(--text-muted)", fontSize: 11 }}>{prompt.description}</div>
                  ) : null}
                </div>
                <div style={{ display: "flex", gap: 4, flexShrink: 0 }}>
                  <button
                    className="btn btn-ghost btn-icon-sm"
                    type="button"
                    title={uiText("编辑", "Edit", "編集")}
                    onClick={() =>
                      setDraft({
                        id: prompt.id,
                        name: prompt.name,
                        description: prompt.description ?? "",
                        content: prompt.content,
                        enabled: prompt.enabled,
                      })
                    }
                  >
                    <Pencil size={14} />
                  </button>
                  <button
                    className="btn btn-ghost btn-icon-sm"
                    type="button"
                    title={uiText("删除", "Delete", "削除")}
                    onClick={() => setPendingDelete(prompt)}
                  >
                    <Trash2 size={14} style={{ color: "var(--danger)" }} />
                  </button>
                </div>
              </div>
              <div
                className="code-block"
                style={{ fontSize: 11, maxHeight: 150, minHeight: 72, overflow: "auto", whiteSpace: "pre-wrap" }}
              >
                {prompt.content || uiText("（空内容）", "(empty)", "（空）")}
              </div>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  gap: 8,
                  alignItems: "center",
                  marginTop: "auto",
                }}
              >
                <span style={{ fontSize: 10, color: "var(--text-muted)" }}>
                  {new Date(prompt.updatedAt).toLocaleString()}
                </span>
                <button
                  className={`btn btn-sm ${prompt.enabled ? "btn-secondary" : "btn-primary"}`}
                  type="button"
                  onClick={() => void activatePrompt(prompt)}
                  disabled={writing}
                >
                  {prompt.enabled ? uiText("重新写入", "Rewrite", "再書き込み") : uiText("启用", "Activate", "有効化")}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <ConfirmDialog
        isOpen={pendingDelete !== null}
        title={uiText("删除 Prompt", "Delete prompt", "Prompt を削除")}
        message={uiText(
          `确定删除“${pendingDelete?.name ?? ""}”吗？已写入工具的文件不会被删除。`,
          `Delete “${pendingDelete?.name ?? ""}”? The live tool file will be preserved.`,
          `「${pendingDelete?.name ?? ""}」を削除しますか？現在のファイルは保持されます。`,
        )}
        confirmText={uiText("删除", "Delete", "削除")}
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        onConfirm={() => void deletePrompt()}
        onCancel={() => setPendingDelete(null)}
      />
    </div>
  );
}
