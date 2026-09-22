import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Layers3, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import { showToast } from "../components/Toast";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { Input } from "../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/ui/select";
import { Textarea } from "../components/ui/textarea";
import { getLocale } from "../lib/i18n";
import {
  buildMcodeProvider,
  createMcodeDraft,
  type McodeApi,
  type McodeDraft,
  type McodeProvider,
  type McodeState,
} from "../lib/mcode";

const API_FORMATS: { value: McodeApi; label: string }[] = [
  { value: "anthropic-messages", label: "Anthropic Messages" },
  { value: "openai-completions", label: "OpenAI Chat Completions" },
  { value: "openai-responses", label: "OpenAI Responses" },
];

function validateDraft(draft: McodeDraft): string | null {
  if (!/^[a-zA-Z0-9_-]+$/.test(draft.id)) return "Provider ID must contain letters, digits, '-' or '_'";
  try {
    const url = new URL(draft.baseUrl);
    if (!["http:", "https:"].includes(url.protocol)) return "Enter an HTTP(S) endpoint";
  } catch {
    return "Enter a valid endpoint URL";
  }
  if (!draft.apiKey.trim()) return "Enter an API key";
  if (!draft.models.split(/[\n,]/).some((model) => model.trim())) return "Add at least one model";
  return null;
}

export default function Mcode() {
  const locale = getLocale();
  const label = (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en);
  const [state, setState] = useState<McodeState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<McodeDraft | null>(null);
  const [originalId, setOriginalId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      setState(await invoke<McodeState>("get_mcode_state"));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const openEditor = (id?: string, provider?: McodeProvider) => {
    setOriginalId(id ?? null);
    setDraft(createMcodeDraft(id, provider));
    setError(null);
  };

  const save = async () => {
    if (!draft || busy) return;
    const issue = validateDraft(draft);
    if (issue) {
      setError(issue);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await invoke("save_mcode_provider", {
        id: draft.id,
        provider: buildMcodeProvider(draft, originalId ? state?.providers[originalId] : undefined),
      });
      setDraft(null);
      await load();
      showToast("success", label("配置已保存", "Provider saved", "プロバイダーを保存しました"));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!deletingId || busy) return;
    setBusy(true);
    setError(null);
    try {
      await invoke("delete_mcode_provider", { id: deletingId });
      setDeletingId(null);
      await load();
      showToast("success", label("配置已删除", "Provider deleted", "プロバイダーを削除しました"));
    } catch (cause) {
      setDeletingId(null);
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const providers = Object.entries(state?.providers ?? {});
  return (
    <div className="page-enter">
      <div className="page-header">
        <div>
          <h2 className="page-title">MiniMax Code</h2>
          <p className="page-subtitle">
            {label(
              `${providers.length} 个自定义供应商`,
              `${providers.length} custom providers`,
              `${providers.length} 件のカスタムプロバイダー`,
            )}
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" onClick={() => void load()} disabled={loading}>
            <RefreshCw size={14} />
            {label("刷新", "Refresh", "更新")}
          </Button>
          <Button size="sm" onClick={() => openEditor()}>
            <Plus size={14} />
            {label("新增供应商", "New provider", "プロバイダーを追加")}
          </Button>
        </div>
      </div>

      {error && !draft && (
        <p role="alert" className="mb-4 text-sm text-[var(--danger)]">
          {error}
        </p>
      )}
      {state && (
        <p className="mb-4 truncate text-xs text-[var(--text-muted)]" title={state.configPath}>
          {state.configPath}
        </p>
      )}
      {loading && !state ? (
        <p className="text-sm text-[var(--text-secondary)]">{label("读取中...", "Loading...", "読み込み中...")}</p>
      ) : (
        <div className="flex flex-col gap-2">
          {providers.length === 0 && (
            <div className="flex min-h-48 flex-col items-center justify-center gap-2 text-[var(--text-secondary)]">
              <Layers3 size={26} aria-hidden="true" />
              <p className="text-sm">
                {label("暂无自定义供应商", "No custom providers", "カスタムプロバイダーはありません")}
              </p>
            </div>
          )}
          {providers.map(([id, provider]) => (
            <div
              key={id}
              className="flex min-w-0 flex-wrap items-center gap-3 rounded-md border border-[var(--border-default)] bg-[var(--bg-card)] px-4 py-3"
            >
              <Layers3 size={16} className="shrink-0 text-[var(--text-muted)]" aria-hidden="true" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 text-sm font-semibold">
                  <span className="truncate">{id}</span>
                  {provider.enabled !== false && (
                    <Check
                      size={13}
                      className="text-[var(--success)]"
                      aria-label={label("已启用", "Enabled", "有効")}
                    />
                  )}
                </div>
                <p className="truncate text-xs text-[var(--text-secondary)]">{provider.options?.baseURL || "-"}</p>
              </div>
              <span className="text-xs text-[var(--text-muted)]">
                {Object.keys(provider.models ?? {}).length} {label("模型", "models", "モデル")}
              </span>
              <span className="hidden text-xs text-[var(--text-muted)] sm:inline">
                {provider.api ?? "anthropic-messages"}
              </span>
              <Button
                variant="ghost"
                size="icon"
                aria-label={label(`编辑 ${id}`, `Edit ${id}`, `${id} を編集`)}
                title={label("编辑", "Edit", "編集")}
                onClick={() => openEditor(id, provider)}
              >
                <Pencil size={15} />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label={label(`删除 ${id}`, `Delete ${id}`, `${id} を削除`)}
                title={label("删除", "Delete", "削除")}
                onClick={() => setDeletingId(id)}
              >
                <Trash2 size={15} />
              </Button>
            </div>
          ))}
        </div>
      )}

      <Dialog
        open={draft !== null}
        onOpenChange={(open) => {
          if (!open && !busy) {
            setDraft(null);
            setError(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <div>
              <DialogTitle>
                {originalId
                  ? label("编辑供应商", "Edit provider", "プロバイダーを編集")
                  : label("新增供应商", "New provider", "プロバイダーを追加")}
              </DialogTitle>
              <DialogDescription>MiniMax Code</DialogDescription>
            </div>
          </DialogHeader>
          {draft && (
            <DialogBody className="space-y-4">
              <label className="block text-xs font-medium">
                Provider ID
                <Input
                  className="mt-1 w-full"
                  value={draft.id}
                  disabled={Boolean(originalId)}
                  onChange={(event) => setDraft({ ...draft, id: event.target.value })}
                />
              </label>
              <label className="block text-xs font-medium">
                API
                <Select value={draft.api} onValueChange={(api: McodeApi) => setDraft({ ...draft, api })}>
                  <SelectTrigger className="mt-1">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {API_FORMATS.map((format) => (
                      <SelectItem key={format.value} value={format.value}>
                        {format.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
              <label className="block text-xs font-medium">
                {label("端点 URL", "Endpoint URL", "エンドポイント URL")}
                <Input
                  className="mt-1 w-full"
                  type="url"
                  value={draft.baseUrl}
                  onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
                />
              </label>
              <label className="block text-xs font-medium">
                API Key
                <Input
                  className="mt-1 w-full"
                  type="password"
                  autoComplete="off"
                  value={draft.apiKey}
                  onChange={(event) => setDraft({ ...draft, apiKey: event.target.value })}
                />
              </label>
              <label className="block text-xs font-medium">
                {label("模型 ID（每行一个）", "Model IDs (one per line)", "モデル ID（1行に1つ）")}
                <Textarea
                  className="mt-1"
                  value={draft.models}
                  onChange={(event) => setDraft({ ...draft, models: event.target.value })}
                />
              </label>
              <Button
                variant={draft.enabled ? "secondary" : "ghost"}
                size="sm"
                aria-pressed={draft.enabled}
                onClick={() => setDraft({ ...draft, enabled: !draft.enabled })}
              >
                <Check size={14} className={draft.enabled ? "opacity-100" : "opacity-0"} />
                {label("启用", "Enabled", "有効")}
              </Button>
              {error && (
                <p role="alert" className="text-xs text-[var(--danger)]">
                  {error}
                </p>
              )}
            </DialogBody>
          )}
          <DialogFooter>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                setDraft(null);
                setError(null);
              }}
              disabled={busy}
            >
              {label("取消", "Cancel", "キャンセル")}
            </Button>
            <Button size="sm" onClick={() => void save()} disabled={busy}>
              {label("保存", "Save", "保存")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        isOpen={deletingId !== null}
        title={label("删除供应商", "Delete provider", "プロバイダーを削除")}
        message={label(`确定删除 ${deletingId}？`, `Delete ${deletingId}?`, `${deletingId} を削除しますか？`)}
        onCancel={() => setDeletingId(null)}
        onConfirm={() => void remove()}
      />
    </div>
  );
}
