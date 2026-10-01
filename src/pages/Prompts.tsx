import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Eye, FileText, Plus, RefreshCw, Search } from "lucide-react";
import ConfirmDialog from "../components/ConfirmDialog";
import CodeEditor from "../components/CodeEditor";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import EmptyState from "../components/states/EmptyState";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { SimpleSelect } from "../components/ui/simple-select";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { fetchVisibleApps } from "../lib/appPreferences";
import { getLocale } from "../lib/i18n";
import { showToast } from "../components/Toast";
import PromptEditor from "./prompts/Editor";
import PromptCard from "./prompts/PromptCard";
import {
  APP_OPTIONS,
  draftFrom,
  emptyDraft,
  invalidDraft,
  type PromptApp,
  type PromptDraft,
  type PromptRecord,
} from "./prompts/types";
import { READBACK_ERROR, usePromptLibrary } from "./prompts/usePromptLibrary";

type DraftSession = { value: PromptDraft; original: PromptDraft; libraryRevision: string; liveRevision?: string };
export default function Prompts() {
  const locale = getLocale();
  const text = useCallback(
    (zh: string, en: string, ja?: string) => (locale === "zh" ? zh : locale === "ja" ? (ja ?? en) : en),
    [locale],
  );
  const [app, setApp] = useState<PromptApp>("claude");
  const [visibleApps, setVisibleApps] = useState<PromptApp[]>(APP_OPTIONS.map((option) => option.id));
  const { snapshot, loading, writing, error, needsReload, refresh, mutate, isWriting } = usePromptLibrary(app);
  const [draft, setDraft] = useState<DraftSession | null>(null);
  const [query, setQuery] = useState("");
  const [pendingDelete, setPendingDelete] = useState<PromptRecord | null>(null);
  const [discard, setDiscard] = useState<(() => void) | null>(null);
  const [previewLive, setPreviewLive] = useState(false);
  const [previewStored, setPreviewStored] = useState(false);
  const [reviewed, setReviewed] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const dirty = !!draft && JSON.stringify(draft.value) !== JSON.stringify(draft.original);
  const canWriteLive = !!snapshot?.live && !needsReload && !loading;
  const disabled = writing || loading || needsReload;
  const option = APP_OPTIONS.find((option) => option.id === app)!;
  const records = useMemo(
    () =>
      Object.values(snapshot?.prompts ?? {})
        .filter((record) =>
          `${record.name}\n${record.description ?? ""}\n${record.content}`
            .toLocaleLowerCase()
            .includes(query.trim().toLocaleLowerCase()),
        )
        .sort((left, right) => Number(right.enabled) - Number(left.enabled) || right.updatedAt - left.updatedAt),
    [snapshot, query],
  );
  const active = Object.values(snapshot?.prompts ?? {}).find((record) => record.enabled);
  const matchesLive = !!active && snapshot?.live?.content === active.content;
  const storedDraft = draft ? snapshot?.prompts[draft.value.id] : undefined;
  const errorMessage =
    error === READBACK_ERROR
      ? text(
          "修改已保存，但暂时无法读取最新状态。请重新加载后再继续。",
          "The change was saved. Reload to read its current state before continuing.",
        )
      : error;

  useEffect(() => {
    let owns = true;
    void fetchVisibleApps()
      .then((apps) => {
        if (!owns) return;
        const supported = APP_OPTIONS.filter((entry) => apps.includes(entry.id)).map((entry) => entry.id);
        if (supported.length) setVisibleApps(supported);
      })
      .catch(() => undefined);
    return () => {
      owns = false;
    };
  }, []);
  const navigate = useCallback(
    (action: () => void) => {
      if (isWriting()) return;
      if (dirty) setDiscard(() => action);
      else action();
    },
    [dirty, isWriting],
  );
  const openDraft = useCallback(
    (value: PromptDraft) => {
      if (!snapshot || loading || isWriting()) return;
      navigate(() => {
        setDraft({
          value,
          original: { ...value },
          libraryRevision: snapshot.libraryRevision,
          liveRevision: snapshot.live?.revision,
        });
        setReviewed(false);
      });
    },
    [snapshot, loading, isWriting, navigate],
  );
  const reload = useCallback(async () => {
    const updated = await refresh();
    if (updated) {
      setDraft((current) =>
        current ? { ...current, libraryRevision: updated.libraryRevision, liveRevision: updated.live?.revision } : null,
      );
      setReviewed(true);
    }
  }, [refresh]);
  const save = useCallback(
    async (activate: boolean) => {
      if (!draft || invalidDraft(draft.value) || isWriting()) return;
      const enabled = activate || draft.value.enabled;
      if (enabled && !canWriteLive) return;
      if (
        await mutate("upsert_prompt", {
          id: draft.value.id,
          prompt: { ...draft.value, enabled },
          expectedLibraryRevision: draft.libraryRevision,
          expectedLiveRevision: draft.liveRevision,
        })
      ) {
        setDraft(null);
        showToast("success", text("已保存", "Saved", "保存しました"));
      }
    },
    [draft, isWriting, canWriteLive, mutate, text],
  );
  useEffect(() => {
    const onNew = () => openDraft(emptyDraft());
    const onSave = () => {
      if (draft) void save(false);
    };
    const onSearch = () => {
      searchRef.current?.focus();
    };
    const onEscape = () => {
      if (isWriting()) return;
      if (previewLive) {
        setPreviewLive(false);
        return;
      }
      if (previewStored) {
        setPreviewStored(false);
        return;
      }
      if (discard) {
        setDiscard(null);
        return;
      }
      if (pendingDelete) {
        setPendingDelete(null);
        return;
      }
      if (draft) {
        navigate(() => setDraft(null));
        return;
      }
      if (query) setQuery("");
    };
    const handlers = { new: onNew, save: onSave, search: onSearch, escape: onEscape };
    for (const [key, handler] of Object.entries(handlers)) window.addEventListener(`cchub-shortcut-${key}`, handler);
    return () => {
      for (const [key, handler] of Object.entries(handlers))
        window.removeEventListener(`cchub-shortcut-${key}`, handler);
    };
  }, [draft, save, openDraft, navigate, isWriting, previewLive, previewStored, discard, pendingDelete, query]);

  if (!snapshot && loading) return <LoadingState label={text("正在加载 Prompt…", "Loading prompts…")} />;
  if (!snapshot)
    return (
      <ErrorState
        title={text("Prompt 加载失败", "Could not load prompts")}
        message={errorMessage ?? ""}
        retryLabel={text("重试", "Retry")}
        onRetry={() => void reload()}
      />
    );
  const liveStatus = snapshot.liveError
    ? text("读取失败", "Read failed")
    : snapshot.live?.content === null
      ? text("文件尚不存在", "No live file")
      : matchesLive
        ? active!.name
        : text("当前文件尚未匹配库内版本", "Live file differs from the selected version");
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <FileText size={19} />
            <h1 className="text-xl font-[590]">
              {draft
                ? text("编辑 Prompt", "Edit prompt", "Prompt を編集")
                : text("Prompt 库", "Prompt Library", "Prompt ライブラリ")}
            </h1>
          </div>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            {text(
              "管理每个工具的指令，替换前保留原有内容。",
              "Manage instructions per tool. Previous content is retained before replacement.",
            )}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="secondary"
            disabled={writing || loading}
            onClick={() => void reload()}
            aria-label={text("刷新", "Refresh", "更新")}
          >
            <RefreshCw size={14} className={loading ? "spin" : undefined} />
            {text("刷新", "Refresh", "更新")}
          </Button>
          {!draft && (
            <Button disabled={writing || loading} onClick={() => openDraft(emptyDraft())}>
              <Plus size={14} />
              {text("新建", "New", "新規")}
            </Button>
          )}
        </div>
      </div>
      <Card className="grid min-w-0 items-center gap-3 p-3 sm:grid-cols-[148px_minmax(0,1fr)] lg:grid-cols-[148px_minmax(0,1fr)_auto]">
        <SimpleSelect
          value={app}
          options={APP_OPTIONS.filter((entry) => visibleApps.includes(entry.id))
            .concat(visibleApps.includes(app) ? [] : [option])
            .map((entry) => ({ value: entry.id, label: entry.label }))}
          ariaLabel={text("工具", "Tool", "ツール")}
          className="w-[148px] shrink-0"
          disabled={writing || loading}
          onValueChange={(next) =>
            navigate(() => {
              setDraft(null);
              setPendingDelete(null);
              setPreviewLive(false);
              setPreviewStored(false);
              setQuery("");
              setApp(next as PromptApp);
              setReviewed(false);
            })
          }
        />
        <div className="min-w-0 flex-1 text-xs">
          <p className="break-all text-muted-foreground">{option.file}</p>
          <p className="mt-1 break-words font-[510]">{liveStatus}</p>
        </div>
        <div className="flex flex-wrap gap-2 sm:col-span-2 lg:col-span-1">
          <Button
            variant="ghost"
            disabled={snapshot.live?.content === null || !snapshot.live}
            onClick={() => setPreviewLive(true)}
          >
            <Eye size={14} />
            {text("查看文件", "View file", "ファイルを表示")}
          </Button>
          {!draft && (
            <Button
              variant="secondary"
              disabled={disabled || snapshot.live?.content === null || !snapshot.live}
              onClick={() => void mutate("import_prompt_from_file")}
            >
              {text("导入当前文件", "Import live file", "現在のファイルをインポート")}
            </Button>
          )}
        </div>
      </Card>
      {(error || snapshot.liveError) && (
        <Card role="alert" className="flex flex-wrap items-center gap-3 p-3 text-xs">
          <div className="min-w-0 flex-1">
            <p className="font-[590]">
              {text("已保留当前内容，请检查后重试", "Current content retained. Review before retrying.")}
            </p>
            <p className="break-words text-muted-foreground">{errorMessage ?? snapshot.liveError}</p>
          </div>
          <Button variant="secondary" disabled={writing || loading} onClick={() => void reload()}>
            {text("重新加载", "Reload", "再読み込み")}
          </Button>
        </Card>
      )}
      {draft && reviewed && (
        <Card role="status" className="flex flex-wrap items-center gap-3 p-3 text-xs leading-5 text-muted-foreground">
          <p className="min-w-0 flex-1">
            {text(
              "已重新读取最新版本，草稿已保留。请查看当前文件再保存；写入前会保留被替换内容。",
              "Latest state loaded and draft retained. Review the live file before saving; replaced content will be retained.",
            )}
          </p>
          {storedDraft ? (
            <Button variant="secondary" onClick={() => setPreviewStored(true)}>
              {text("查看库内版本", "View stored version")}
            </Button>
          ) : (
            <p>
              {text(
                "这份草稿不在当前库中，保存后会创建新版本。",
                "This draft is absent from the library. Saving will create it.",
              )}
            </p>
          )}
        </Card>
      )}
      {draft ? (
        <PromptEditor
          key={draft.value.id}
          draft={draft.value}
          text={text}
          writing={writing}
          blocked={needsReload || loading}
          canWriteLive={canWriteLive}
          onChange={(value) => setDraft((current) => (current ? { ...current, value } : null))}
          onClose={() => navigate(() => setDraft(null))}
          onSave={(activate) => void save(activate)}
        />
      ) : (
        <>
          <div className="relative max-w-[420px]">
            <Search
              size={14}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              ref={searchRef}
              className="w-full"
              style={{ paddingLeft: 34 }}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              aria-label={text("搜索 Prompt", "Search prompts", "Prompt を検索")}
              placeholder={text("搜索名称、说明或内容…", "Search name, description or content…")}
            />
          </div>
          {!records.length ? (
            <EmptyState
              title={query ? text("没有匹配的 Prompt", "No matching prompts") : text("暂无 Prompt", "No prompts")}
              description={
                query
                  ? text("尝试其他关键词。", "Try another search.")
                  : text("新建指令版本，或导入当前文件。", "Create instructions or import the live file.")
              }
              action={
                <Button
                  variant="secondary"
                  onClick={() => (query ? setQuery("") : openDraft(emptyDraft()))}
                  disabled={writing || loading}
                >
                  {query ? text("清除搜索", "Clear search") : text("新建 Prompt", "New prompt")}
                </Button>
              }
            />
          ) : (
            <div className="grid min-w-0 gap-3 md:grid-cols-2 xl:grid-cols-3">
              {records.map((record) => (
                <PromptCard
                  key={record.id}
                  prompt={record}
                  matchesLive={record.enabled && matchesLive}
                  disabled={writing || loading}
                  canWriteLive={canWriteLive}
                  text={text}
                  onEdit={() => openDraft(draftFrom(record))}
                  onDelete={() => setPendingDelete(record)}
                  onActivate={() => void mutate("enable_prompt", { id: record.id })}
                />
              ))}
            </div>
          )}
        </>
      )}
      <ConfirmDialog
        isOpen={!!discard}
        title={text("放弃未保存的修改？", "Discard unsaved changes?")}
        message={text("关闭后，这份草稿的修改将丢失。", "Closing will discard this draft.")}
        confirmText={text("放弃修改", "Discard")}
        cancelText={text("继续编辑", "Keep editing")}
        onConfirm={() => {
          if (!isWriting()) {
            discard?.();
            setDiscard(null);
          }
        }}
        onCancel={() => setDiscard(null)}
      />
      <ConfirmDialog
        isOpen={!!pendingDelete}
        title={text("删除 Prompt", "Delete prompt")}
        message={text(
          `删除“${pendingDelete?.name ?? ""}”？工具的当前文件会保留。`,
          `Delete “${pendingDelete?.name ?? ""}”? The live file will be preserved.`,
        )}
        confirmText={text("删除", "Delete")}
        cancelText={text("取消", "Cancel")}
        onConfirm={() => {
          if (!pendingDelete || isWriting()) return;
          const id = pendingDelete.id;
          setPendingDelete(null);
          void mutate("delete_prompt", { id });
        }}
        onCancel={() => {
          if (!isWriting()) setPendingDelete(null);
        }}
      />
      <Dialog open={previewLive} onOpenChange={setPreviewLive}>
        <DialogContent className="max-w-[720px]">
          <DialogHeader>
            <div className="min-w-0">
              <DialogTitle>{text("当前指令文件", "Live instruction file")}</DialogTitle>
              <DialogDescription className="break-all">{option.file}</DialogDescription>
            </div>
          </DialogHeader>
          <DialogBody>
            <CodeEditor value={snapshot.live?.content ?? ""} language="markdown" readOnly minHeight={200} />
          </DialogBody>
        </DialogContent>
      </Dialog>
      <Dialog open={previewStored} onOpenChange={setPreviewStored}>
        <DialogContent className="max-w-[720px]">
          <DialogHeader>
            <div className="min-w-0">
              <DialogTitle>{text("库内当前版本", "Current stored version")}</DialogTitle>
              <DialogDescription className="break-words">{storedDraft?.name}</DialogDescription>
            </div>
          </DialogHeader>
          <DialogBody>
            <CodeEditor value={storedDraft?.content ?? ""} language="markdown" readOnly minHeight={200} />
          </DialogBody>
        </DialogContent>
      </Dialog>
    </div>
  );
}
