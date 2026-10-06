import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, Search } from "lucide-react";
import { getLocale } from "../../lib/i18n";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import MarkdownPreview from "../MarkdownPreview";

export interface MemoryEntry {
  path: string;
  file_name: string;
  source: string;
  project_name: string | null;
  modified_at: string | null;
  preview: string;
}
export default function MemoryBrowser() {
  const zh = getLocale() === "zh";
  const text = (zhText: string, en: string) => (zh ? zhText : en);
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<MemoryEntry[]>([]);
  const [selected, setSelected] = useState<MemoryEntry | null>(null);
  const [content, setContent] = useState("");
  const [loading, setLoading] = useState(true);
  const [reading, setReading] = useState(false);
  const [searchError, setSearchError] = useState(false);
  const [contentError, setContentError] = useState(false);
  const selection = useRef<MemoryEntry | null>(null);
  const searchGeneration = useRef(0);
  const contentGeneration = useRef(0);
  const open = useCallback(async (entry: MemoryEntry) => {
    const request = ++contentGeneration.current;
    selection.current = entry;
    setSelected(entry);
    setContent("");
    setReading(true);
    setContentError(false);
    try {
      const value = await invoke<string>("read_openclaw_daily_memory_content", { path: entry.path });
      if (request === contentGeneration.current) setContent(value);
    } catch {
      if (request === contentGeneration.current) setContentError(true);
    } finally {
      if (request === contentGeneration.current) setReading(false);
    }
  }, []);
  const search = useCallback(
    async (value: string) => {
      const request = ++searchGeneration.current;
      const previousSelection = contentGeneration.current;
      setLoading(true);
      setSearchError(false);
      try {
        const next = await invoke<MemoryEntry[]>("search_openclaw_daily_memory", { query: value.trim(), limit: 40 });
        if (request !== searchGeneration.current) return;
        setEntries(next);
        // A user selection made while searching takes precedence over auto-selection.
        if (previousSelection !== contentGeneration.current) return;
        const retained = next.find((entry) => entry.path === selection.current?.path);
        if (retained) {
          setSelected(retained);
          selection.current = retained;
        } else if (next[0]) void open(next[0]);
        else {
          contentGeneration.current++;
          selection.current = null;
          setSelected(null);
          setContent("");
          setReading(false);
          setContentError(false);
        }
      } catch {
        if (request === searchGeneration.current) setSearchError(true);
      } finally {
        if (request === searchGeneration.current) setLoading(false);
      }
    },
    [open],
  );
  useEffect(() => {
    const searches = searchGeneration;
    const reads = contentGeneration;
    void search("");
    return () => {
      searches.current++;
      reads.current++;
    };
  }, [search]);
  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-3">
      <form
        className="flex min-w-0 gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          void search(query);
        }}
      >
        <Input
          aria-label={text("搜索记忆", "Search memory")}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={text("关键词；留空显示最近记录", "Keywords, or leave empty for recent entries")}
          onKeyDown={(event) => {
            if (event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault();
          }}
        />
        <Button type="submit" variant="secondary" aria-label={text("搜索记忆记录", "Search memory entries")}>
          <Search size={14} />
        </Button>
        <Button
          type="button"
          variant="ghost"
          aria-label={text("刷新记忆结果", "Refresh memory results")}
          onClick={() => void search(query)}
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : undefined} />
        </Button>
      </form>
      {searchError && (
        <div role="alert" className="rounded-md border border-destructive/30 p-3 text-xs">
          {text("搜索失败，已保留上次结果。", "Search failed. Previous results are retained.")}
          <Button variant="ghost" onClick={() => void search(query)}>
            {text("重试", "Retry")}
          </Button>
        </div>
      )}
      <div className="grid min-h-0 min-w-0 flex-1 grid-rows-[auto_minmax(220px,1fr)] gap-3 overflow-auto md:grid-cols-[minmax(220px,0.8fr)_minmax(0,1.4fr)] md:grid-rows-1">
        <div
          className="flex max-h-[260px] min-w-0 flex-col gap-1 overflow-y-auto rounded-lg border border-border p-2 md:max-h-none"
          aria-busy={loading}
          aria-label={text("记忆结果", "Memory results")}
        >
          {!entries.length && (
            <p role="status" className="p-3 text-xs text-muted-foreground">
              {text(
                loading ? "正在搜索…" : searchError ? "请重试搜索" : "没有匹配的记忆记录",
                loading ? "Searching…" : searchError ? "Retry the search" : "No matching memory entries",
              )}
            </p>
          )}
          {entries.map((entry) => (
            <Button
              key={entry.path}
              type="button"
              variant={selected?.path === entry.path ? "secondary" : "ghost"}
              className="h-auto min-w-0 flex-col items-start gap-1 whitespace-normal px-3 py-3 text-left"
              aria-pressed={selected?.path === entry.path}
              onClick={() => void open(entry)}
            >
              <span className="w-full break-all text-xs font-semibold">{entry.file_name}</span>
              <span className="w-full break-words text-[11px] text-muted-foreground">
                {entry.source === "global" ? text("全局", "Global") : entry.project_name || text("项目", "Project")}
              </span>
              {entry.modified_at && (
                <span className="w-full text-[11px] text-muted-foreground">{entry.modified_at}</span>
              )}
              <span className="line-clamp-2 w-full break-all text-xs font-normal text-muted-foreground">
                {entry.preview}
              </span>
            </Button>
          ))}
        </div>
        <section
          className="flex min-h-[220px] min-w-0 flex-col overflow-hidden rounded-lg border border-border"
          aria-label={text("记忆内容", "Memory content")}
          aria-busy={reading}
        >
          <div className="shrink-0 border-b border-border p-3">
            <h3 className="break-all text-xs font-semibold">
              {selected?.file_name || text("全文预览", "Full preview")}
            </h3>
            {selected && <p className="mt-1 break-all text-[11px] text-muted-foreground">{selected.path}</p>}
          </div>
          <div className="min-h-0 min-w-0 flex-1 overflow-auto p-4">
            {reading ? (
              <p role="status" className="text-xs text-muted-foreground">
                {text("正在读取…", "Reading…")}
              </p>
            ) : contentError ? (
              <div role="alert" className="text-xs">
                {text("读取失败，可以重试当前记录。", "Reading failed. Retry this entry.")}
                <Button variant="secondary" onClick={() => selected && void open(selected)}>
                  {text("重试读取", "Retry reading")}
                </Button>
              </div>
            ) : selected ? (
              content ? (
                <MarkdownPreview content={content} loadingLabel={text("加载预览…", "Loading preview…")} />
              ) : (
                <p className="text-xs text-muted-foreground">{text("此记录为空", "This entry is empty")}</p>
              )
            ) : (
              <p className="text-xs text-muted-foreground">
                {text("选择一条记录查看内容。", "Select an entry to read its content.")}
              </p>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
