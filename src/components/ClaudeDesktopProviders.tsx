import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check, Pencil, Plus, RotateCcw, Trash2 } from "lucide-react";
import ConfirmDialog from "./ConfirmDialog";
import { showToast } from "./Toast";
import { Button } from "./ui/button";
import { Dialog, DialogBody, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Input } from "./ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Textarea } from "./ui/textarea";
import type { Locale } from "../lib/i18n";

interface DirectProvider {
  id: string;
  name: string;
  baseUrl: string;
  hasApiKey: boolean;
  models: string[];
  mode?: "direct" | "proxy";
  apiFormat?: string;
  modelRoutes?: Record<string, string>;
}

interface ProviderState {
  providers: DirectProvider[];
  activeId: string | null;
  profilePath: string;
}

interface Draft {
  id?: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  models: string;
  mode: "direct" | "proxy";
  apiFormat: string;
  modelRoutes: string;
}

function parseRoutes(raw: string): Record<string, string> {
  return Object.fromEntries(
    raw
      .split("\n")
      .map((line) => line.split("=", 2).map((part) => part.trim()))
      .filter(([route, model]) => route && model),
  );
}

function validateDraft(draft: Draft): string | null {
  if (!draft.name.trim()) return "Enter a provider name";
  try {
    const url = new URL(draft.baseUrl);
    if (
      url.protocol !== "https:" &&
      !(url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname))
    ) {
      return "Use HTTPS or a loopback HTTP gateway";
    }
    if (url.username || url.password || url.hash) return "Remove credentials and fragments from the URL";
  } catch {
    return "Enter a valid gateway URL";
  }
  if (!draft.apiKey.trim() && !draft.id) return "Enter a gateway API key";
  const models = (draft.mode === "proxy" ? Object.keys(parseRoutes(draft.modelRoutes)).join(",") : draft.models)
    .split(/[\n,]/)
    .map((model) => model.trim())
    .filter(Boolean);
  if (models.some((model) => !/^claude-(sonnet|opus|haiku|fable)-[a-z0-9-]+$/i.test(model))) {
    return "Use Claude Sonnet, Opus, Haiku or Fable model IDs";
  }
  if (draft.mode === "proxy") {
    if (
      !draft.modelRoutes.trim() ||
      draft.modelRoutes.split("\n").some((line) => line.trim() && !/^[^=]+=[^=]+$/.test(line.trim()))
    ) {
      return "Map each Claude model to an upstream model using model=upstream";
    }
    if (!models.length) return "Add at least one model route";
  }
  return null;
}

export default function ClaudeDesktopProviders({ locale }: { locale: Locale }) {
  const label = (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en);
  const [state, setState] = useState<ProviderState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [confirm, setConfirm] = useState<{ action: "delete" | "restore"; id?: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setState(await invoke<ProviderState>("get_claude_desktop_providers"));
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (command: string, args?: Record<string, unknown>) => {
    if (busy) return false;
    setBusy(true);
    setError(null);
    try {
      setState(await invoke<ProviderState>(command, args));
      showToast(
        "success",
        label(
          "Claude Desktop 配置已更新",
          "Claude Desktop configuration updated",
          "Claude Desktop の設定を更新しました",
        ),
      );
      return true;
    } catch (cause) {
      setError(String(cause));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    if (!draft) return;
    const issue = validateDraft(draft);
    if (issue) {
      setError(issue);
      return;
    }
    const saved = await run("save_claude_desktop_provider", {
      id: draft.id ?? null,
      name: draft.name,
      baseUrl: draft.baseUrl,
      apiKey: draft.apiKey,
      models:
        draft.mode === "proxy"
          ? Object.keys(parseRoutes(draft.modelRoutes))
          : draft.models
              .split(/[\n,]/)
              .map((model) => model.trim())
              .filter(Boolean),
      mode: draft.mode,
      apiFormat: draft.mode === "proxy" ? draft.apiFormat : "anthropic",
      modelRoutes: draft.mode === "proxy" ? parseRoutes(draft.modelRoutes) : {},
    });
    if (saved) setDraft(null);
  };

  return (
    <section className="space-y-3" aria-label="Claude Desktop providers">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-semibold">
            {label("Claude Desktop 供应商", "Claude Desktop providers", "Claude Desktop プロバイダー")}
          </h3>
          <p className="mt-1 text-xs text-[var(--text-muted)]">
            {label("直连 / 本地代理", "Direct / local proxy", "直接 / ローカルプロキシ")}
            {state?.activeId
              ? ` · ${label("第三方模式", "Third-party mode", "サードパーティモード")}`
              : ` · ${label("官方模式", "Official mode", "公式モード")}`}
          </p>
        </div>
        <div className="flex gap-2">
          {state?.activeId && (
            <Button variant="secondary" size="sm" onClick={() => setConfirm({ action: "restore" })} disabled={busy}>
              <RotateCcw size={14} />
              {label("恢复官方", "Restore official", "公式に戻す")}
            </Button>
          )}
          <Button
            size="sm"
            onClick={() => {
              setError(null);
              setDraft({
                name: "",
                baseUrl: "",
                apiKey: "",
                models: "",
                mode: "direct",
                apiFormat: "anthropic",
                modelRoutes: "",
              });
            }}
          >
            <Plus size={14} />
            {label("新增供应商", "New provider", "プロバイダーを追加")}
          </Button>
        </div>
      </div>
      {error && !draft && (
        <p role="alert" className="text-xs text-[var(--danger)]">
          {error}
        </p>
      )}
      {state?.providers.length === 0 && (
        <p className="py-8 text-center text-xs text-[var(--text-muted)]">
          {label(
            "暂无 Claude Desktop 直连供应商",
            "No Claude Desktop direct providers",
            "直接プロバイダーはありません",
          )}
        </p>
      )}
      <div className="space-y-2">
        {state?.providers.map((provider) => (
          <div
            key={provider.id}
            className="flex min-w-0 flex-wrap items-center gap-3 rounded-md border border-[var(--border-default)] bg-[var(--bg-card)] px-4 py-3"
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 text-sm font-medium">
                <span className="truncate">{provider.name}</span>
                <span className="shrink-0 text-[11px] font-normal text-[var(--text-muted)]">
                  {provider.mode === "proxy" ? label("代理", "Proxy", "プロキシ") : label("直连", "Direct", "直接")}
                </span>
                {state.activeId === provider.id && (
                  <Check
                    size={14}
                    className="shrink-0 text-[var(--success)]"
                    aria-label={label("已启用", "Active", "有効")}
                  />
                )}
              </div>
              <p className="truncate text-xs text-[var(--text-muted)]" title={provider.baseUrl}>
                {provider.baseUrl}
              </p>
            </div>
            <span className="text-xs text-[var(--text-muted)]">
              {provider.models.length} {label("模型", "models", "モデル")}
            </span>
            {state.activeId !== provider.id && (
              <Button
                size="sm"
                variant="secondary"
                disabled={busy}
                onClick={() => void run("apply_claude_desktop_provider", { id: provider.id })}
              >
                {label("启用", "Activate", "有効にする")}
              </Button>
            )}
            <Button
              size="icon"
              variant="ghost"
              aria-label={label(`编辑 ${provider.name}`, `Edit ${provider.name}`)}
              title={label("编辑", "Edit")}
              onClick={() => {
                setError(null);
                setDraft({
                  ...provider,
                  apiKey: "",
                  models: provider.models.join("\n"),
                  mode: provider.mode ?? "direct",
                  apiFormat: provider.apiFormat ?? "anthropic",
                  modelRoutes: Object.entries(provider.modelRoutes ?? {})
                    .map(([route, upstream]) => `${route}=${upstream}`)
                    .join("\n"),
                });
              }}
            >
              <Pencil size={15} />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              disabled={busy || state.activeId === provider.id}
              aria-label={label(`删除 ${provider.name}`, `Delete ${provider.name}`)}
              title={label("删除", "Delete")}
              onClick={() => setConfirm({ action: "delete", id: provider.id })}
            >
              <Trash2 size={15} />
            </Button>
          </div>
        ))}
      </div>
      {state && (
        <p className="truncate text-[11px] text-[var(--text-muted)]" title={state.profilePath}>
          {state.profilePath}
        </p>
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
            <DialogTitle>
              {draft?.id
                ? label("编辑供应商", "Edit provider", "プロバイダーを編集")
                : label("新增供应商", "New provider", "プロバイダーを追加")}
            </DialogTitle>
          </DialogHeader>
          {draft && (
            <DialogBody className="space-y-4">
              <label className="block text-xs font-medium">
                {label("名称", "Name", "名前")}
                <Input
                  className="mt-1"
                  autoFocus
                  value={draft.name}
                  onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                />
              </label>
              <div
                className="flex gap-1 rounded-md border border-[var(--border-default)] p-1"
                role="group"
                aria-label={label("连接模式", "Connection mode", "接続モード")}
              >
                <Button
                  type="button"
                  className="flex-1"
                  size="sm"
                  variant={draft.mode === "direct" ? "secondary" : "ghost"}
                  aria-pressed={draft.mode === "direct"}
                  onClick={() => setDraft({ ...draft, mode: "direct" })}
                >
                  {label("直连", "Direct", "直接")}
                </Button>
                <Button
                  type="button"
                  className="flex-1"
                  size="sm"
                  variant={draft.mode === "proxy" ? "secondary" : "ghost"}
                  aria-pressed={draft.mode === "proxy"}
                  onClick={() => setDraft({ ...draft, mode: "proxy" })}
                >
                  {label("本地代理", "Local proxy", "ローカルプロキシ")}
                </Button>
              </div>
              {draft.mode === "proxy" && (
                <label className="block text-xs font-medium">
                  API
                  <Select value={draft.apiFormat} onValueChange={(apiFormat) => setDraft({ ...draft, apiFormat })}>
                    <SelectTrigger className="mt-1">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="anthropic">Anthropic Messages</SelectItem>
                      <SelectItem value="openai_chat">OpenAI Chat Completions</SelectItem>
                      <SelectItem value="openai_responses">OpenAI Responses</SelectItem>
                      <SelectItem value="gemini_native">Gemini Native</SelectItem>
                    </SelectContent>
                  </Select>
                </label>
              )}
              <label className="block text-xs font-medium">
                {draft.mode === "proxy"
                  ? label("上游 URL", "Upstream URL", "上流 URL")
                  : label("网关 URL", "Gateway URL", "ゲートウェイ URL")}
                <Input
                  className="mt-1"
                  type="url"
                  placeholder="https://gateway.example.com"
                  value={draft.baseUrl}
                  onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })}
                />
              </label>
              <label className="block text-xs font-medium">
                API Key
                <Input
                  className="mt-1"
                  type="password"
                  autoComplete="new-password"
                  placeholder={draft.id ? label("留空保留原密钥", "Leave blank to keep current key") : ""}
                  value={draft.apiKey}
                  onChange={(event) => setDraft({ ...draft, apiKey: event.target.value })}
                />
              </label>
              {draft.mode === "direct" ? (
                <label className="block text-xs font-medium">
                  {label(
                    "模型 ID（逗号分隔，可选）",
                    "Model IDs (comma-separated, optional)",
                    "モデル ID（カンマ区切り、任意）",
                  )}
                  <Input
                    className="mt-1"
                    placeholder="claude-sonnet-4-6"
                    value={draft.models}
                    onChange={(event) => setDraft({ ...draft, models: event.target.value })}
                  />
                </label>
              ) : (
                <label className="block text-xs font-medium">
                  {label(
                    "模型路由（每行 Claude ID=上游 ID）",
                    "Model routes (Claude ID=upstream ID per line)",
                    "モデルルート（行ごとに Claude ID=上流 ID）",
                  )}
                  <Textarea
                    className="mt-1 min-h-24 font-mono"
                    placeholder="claude-sonnet-4-6=upstream-model"
                    value={draft.modelRoutes}
                    onChange={(event) => setDraft({ ...draft, modelRoutes: event.target.value })}
                  />
                </label>
              )}
              {error && (
                <p role="alert" className="text-xs text-[var(--danger)]">
                  {error}
                </p>
              )}
            </DialogBody>
          )}
          <DialogFooter>
            <Button variant="secondary" onClick={() => setDraft(null)} disabled={busy}>
              {label("取消", "Cancel", "キャンセル")}
            </Button>
            <Button onClick={() => void save()} disabled={busy}>
              {label("保存", "Save", "保存")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <ConfirmDialog
        isOpen={confirm !== null}
        title={
          confirm?.action === "restore"
            ? label("恢复官方模式", "Restore official mode")
            : label("删除供应商", "Delete provider")
        }
        message={
          confirm?.action === "restore"
            ? label(
                "将 Claude Desktop 切回官方模式，保留其他配置和 MCP 服务。",
                "Switch Claude Desktop to official mode while keeping other settings and MCP servers.",
              )
            : label("删除这条未启用的供应商配置？", "Delete this inactive provider?")
        }
        confirmText={label("确认", "Confirm", "確認")}
        cancelText={label("取消", "Cancel", "キャンセル")}
        variant={confirm?.action === "restore" ? "info" : "destructive"}
        onConfirm={() => {
          const current = confirm;
          setConfirm(null);
          if (current)
            void run(
              current.action === "restore" ? "restore_claude_desktop_official" : "delete_claude_desktop_provider",
              current.id ? { id: current.id } : undefined,
            );
        }}
        onCancel={() => setConfirm(null)}
      />
    </section>
  );
}
