import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Copy,
  FolderOpen,
  History,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { getLocale } from "../lib/i18n";
import { fetchVisibleApps, getAppLabel, type ManagedAppId } from "../lib/appPreferences";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import SessionListItem from "../components/SessionListItem";

interface SessionSummary {
  id: string;
  tool_id: string;
  tool_name: string;
  title: string;
  cwd: string | null;
  source_kind: string;
  source_backend: string;
  source_path: string;
  created_at: string | null;
  updated_at: string | null;
  preview: string;
  message_count: number;
  search_hit_count: number;
  can_resume: boolean;
  can_delete: boolean;
}

interface SessionEntry {
  id: string;
  kind: string;
  title: string;
  content: string;
  timestamp: string | null;
}

interface SessionDetail {
  session: SessionSummary;
  entries: SessionEntry[];
}

const TOOL_ORDER: ManagedAppId[] = ["claude", "codex", "gemini", "opencode", "openclaw"];

function countSessionHits(session: SessionSummary, query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return 0;

  return [
    session.title,
    session.preview,
    session.cwd ?? "",
    session.tool_name,
    session.source_backend,
    session.source_path,
  ].filter((value) => value.toLowerCase().includes(normalized)).length;
}

function matchesEntry(entry: SessionEntry, query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  return (
    entry.title.toLowerCase().includes(normalized)
    || entry.kind.toLowerCase().includes(normalized)
    || entry.content.toLowerCase().includes(normalized)
  );
}

/** Build the CLI resume command for a given tool + session ID. */
function buildResumeCommand(toolId: string, sessionId: string): string | null {
  switch (toolId) {
    case "claude":
      return `claude --resume ${sessionId}`;
    case "codex":
      return `codex resume ${sessionId}`;
    case "gemini":
      return `gemini --resume ${sessionId}`;
    case "opencode":
      return `opencode session resume ${sessionId}`;
    default:
      return null; // openclaw etc. — no CLI resume
  }
}

function entryBadgeColor(kind: string) {
  switch (kind) {
    case "user":
      return "badge-accent";
    case "assistant":
      return "badge-success";
    case "tool_call":
      return "badge-muted";
    case "tool_output":
      return "badge-muted";
    case "reasoning":
      return "badge-accent";
    default:
      return "badge-muted";
  }
}

export default function Sessions() {
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>(TOOL_ORDER);
  const [filterTool, setFilterTool] = useState<ManagedAppId | "all">("all");
  const [query, setQuery] = useState("");
  const [allSessions, setAllSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedSession, setSelectedSession] = useState<SessionSummary | null>(null);
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailQuery, setDetailQuery] = useState("");
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<SessionSummary | null>(null);
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const sessionsRequestIdRef = useRef(0);
  const detailRequestIdRef = useRef(0);
  const selectedSessionRef = useRef<SessionSummary | null>(null);
  const detailRef = useRef<SessionDetail | null>(null);
  const deferredQuery = useDeferredValue(query);
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  useEffect(() => {
    void fetchVisibleApps().then(setVisibleApps).catch(() => setVisibleApps(TOOL_ORDER));
    void loadSessions(true);
  }, []);

  useEffect(() => {
    selectedSessionRef.current = selectedSession;
  }, [selectedSession]);

  useEffect(() => {
    detailRef.current = detail;
  }, [detail]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadSessions(false);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [filterTool]);

  useEffect(() => {
    const handleSearch = () => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };
    const handleEscape = () => {
      if (pendingDelete) {
        setPendingDelete(null);
        return;
      }
      if (detailQuery) {
        setDetailQuery("");
        return;
      }
      if (selectedSession) {
        setSelectedSession(null);
        setDetail(null);
      }
    };
    window.addEventListener("cchub-shortcut-search", handleSearch);
    window.addEventListener("cchub-shortcut-escape", handleEscape);
    return () => {
      window.removeEventListener("cchub-shortcut-search", handleSearch);
      window.removeEventListener("cchub-shortcut-escape", handleEscape);
    };
  }, [detailQuery, pendingDelete, selectedSession]);

  async function loadSessions(showLoading: boolean) {
    const requestId = sessionsRequestIdRef.current + 1;
    sessionsRequestIdRef.current = requestId;

    if (showLoading) {
      setLoading(true);
    } else {
      setRefreshing(true);
    }
    try {
      const nextSessions = await invoke<SessionSummary[]>("get_sessions", {
        toolId: filterTool === "all" ? null : filterTool,
        query: null,
        limit: 240,
      });
      if (requestId !== sessionsRequestIdRef.current) {
        return;
      }
      setAllSessions(nextSessions);

      const currentSelected = selectedSessionRef.current;
      if (currentSelected) {
        const nextSelected = nextSessions.find((item) => item.id === currentSelected.id && item.tool_id === currentSelected.tool_id) ?? null;
        setSelectedSession(nextSelected);
        if (nextSelected) {
          const currentDetail = detailRef.current;
          const isSameDetail =
            currentDetail?.session.id === nextSelected.id
            && currentDetail.session.tool_id === nextSelected.tool_id;

          if (isSameDetail) {
            setDetail((current) => (
              current
                ? {
                    ...current,
                    session: {
                      ...current.session,
                      ...nextSelected,
                    },
                  }
                : current
            ));
          } else {
            void openSession(nextSelected, false);
          }
        } else {
          detailRequestIdRef.current += 1;
          setDetailLoading(false);
          setDetail(null);
        }
      }
    } catch (error) {
      if (requestId !== sessionsRequestIdRef.current) {
        return;
      }
      showToast("error", String(error));
      setAllSessions([]);
    } finally {
      if (requestId === sessionsRequestIdRef.current) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }

  const openSession = useCallback(async (session: SessionSummary, updateSelection = true) => {
    const requestId = detailRequestIdRef.current + 1;
    detailRequestIdRef.current = requestId;

    if (updateSelection) {
      setSelectedSession(session);
    }
    setDetailLoading(true);
    try {
      const nextDetail = await invoke<SessionDetail>("get_session_detail", {
        toolId: session.tool_id,
        sessionId: session.id,
        sourcePath: session.source_path,
        sourceKind: session.source_kind,
        sourceBackend: session.source_backend,
        cwd: session.cwd,
        title: session.title,
        preview: session.preview,
        createdAt: session.created_at,
        updatedAt: session.updated_at,
        messageCount: session.message_count,
        canResume: session.can_resume,
        canDelete: session.can_delete,
      });
      if (requestId !== detailRequestIdRef.current) {
        return;
      }
      setDetail(nextDetail);
      setDetailQuery("");
    } catch (error) {
      if (requestId !== detailRequestIdRef.current) {
        return;
      }
      showToast("error", String(error));
      setDetail(null);
    } finally {
      if (requestId === detailRequestIdRef.current) {
        setDetailLoading(false);
      }
    }
  }, []);

  const confirmDeleteSession = useCallback(async () => {
    if (!pendingDelete) return;
    setDeletingId(pendingDelete.id);
    try {
      await invoke("delete_session", {
        toolId: pendingDelete.tool_id,
        sessionId: pendingDelete.id,
        sourcePath: pendingDelete.source_path,
        sourceBackend: pendingDelete.source_backend,
      });
      if (selectedSession?.id === pendingDelete.id && selectedSession.tool_id === pendingDelete.tool_id) {
        setSelectedSession(null);
        setDetail(null);
      }
      setPendingDelete(null);
      await loadSessions(false);
      showToast(
        "success",
        uiText("会话已删除", "Session deleted", "会話を削除しました"),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setDeletingId(null);
    }
  }, [pendingDelete, selectedSession]);

  const handleOpenSession = useCallback((session: SessionSummary) => {
    void openSession(session);
  }, [openSession]);

  const handleCopyResumeCommand = useCallback((command: string) => {
    void navigator.clipboard.writeText(command).then(() =>
      showToast("success", uiText("已复制恢复命令", "Resume command copied", "復元コマンドをコピーしました")),
    );
  }, [uiText]);

  const handleRequestDelete = useCallback((session: SessionSummary) => {
    setPendingDelete(session);
  }, []);

  const toolFilters = useMemo<Array<{ id: ManagedAppId | "all"; label: string }>>(
    () => [
      { id: "all" as const, label: uiText("全部 App", "All Apps", "すべての App") },
      ...TOOL_ORDER.filter((toolId) => visibleApps.includes(toolId)).map((toolId) => ({
        id: toolId,
        label: getAppLabel(toolId),
      })),
    ],
    [uiText, visibleApps],
  );

  const filteredEntries = useMemo(
    () => (detail?.entries || []).filter((entry) => matchesEntry(entry, detailQuery)),
    [detail?.entries, detailQuery],
  );
  const sessions = useMemo(() => {
    const normalized = deferredQuery.trim().toLowerCase();
    if (!normalized) {
      return allSessions;
    }

    return allSessions
      .map((session) => {
        const searchHitCount = countSessionHits(session, normalized);
        return searchHitCount > 0
          ? {
              ...session,
              search_hit_count: searchHitCount,
            }
          : null;
      })
      .filter((session): session is SessionSummary => session !== null);
  }, [allSessions, deferredQuery]);

  if (loading) {
    return (
      <div className="loading-center">
        <div className="spinner" />
        <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
          {uiText("加载会话中...", "Loading sessions...", "会話を読み込み中...")}
        </span>
      </div>
    );
  }

  return (
    <>
      <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column", gap: 16 }}>
        <div className="page-header">
          <div>
            <h2 className="page-title">{uiText("会话管理器", "Sessions", "セッション")}</h2>
            <p className="page-subtitle">
              {uiText(
                "跨 App 浏览、搜索、删除和恢复本地 CLI 会话",
                "Browse, search, delete, and resume local CLI sessions across apps",
                "複数 App のローカル CLI 会話を横断して閲覧・検索・削除・復元します",
              )}
            </p>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={() => void loadSessions(false)}>
            <RefreshCw size={14} className={refreshing ? "spin" : undefined} />
            {uiText("刷新", "Refresh", "更新")}
          </button>
        </div>

        <div className="section-card" style={{ padding: 16 }}>
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
            <div style={{ flex: "1 1 320px", minWidth: 240, position: "relative" }}>
              <Search size={14} style={{ position: "absolute", top: 11, left: 12, color: "var(--text-muted)" }} />
              <input
                ref={searchInputRef}
                className="input"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={uiText("搜索标题、路径或摘要...", "Search title, path, or preview...", "タイトル・パス・要約を検索...")}
                style={{ paddingLeft: 34 }}
              />
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {toolFilters.map((option) => (
                <button
                  key={option.id}
                  className={`btn btn-sm ${filterTool === option.id ? "btn-primary" : "btn-secondary"}`}
                  onClick={() => setFilterTool(option.id)}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "minmax(320px, 420px) minmax(0, 1fr)", gap: 16, flex: 1, minHeight: 0 }}>
          <div className="section-card" style={{ display: "flex", flexDirection: "column", minHeight: 0 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <History size={15} style={{ color: "var(--text-secondary)" }} />
                <span style={{ fontSize: 12, fontWeight: 700, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                  {uiText("会话列表", "Session List", "会話一覧")}
                </span>
              </div>
              <span className="badge badge-muted" style={{ fontSize: 10 }}>{sessions.length}</span>
            </div>

            <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
              {sessions.length === 0 ? (
                <div style={{ display: "flex", alignItems: "center", justifyContent: "center", flex: 1, color: "var(--text-muted)", fontSize: 13 }}>
                  {uiText("当前没有匹配的会话", "No sessions matched the current filters", "現在の条件に一致する会話はありません")}
                </div>
              ) : sessions.map((session) => (
                <SessionListItem
                  key={`${session.tool_id}-${session.id}-${session.source_path}`}
                  session={session}
                  selected={selectedSession?.id === session.id && selectedSession.tool_id === session.tool_id}
                  query={query}
                  resumeCommand={buildResumeCommand(session.tool_id, session.id)}
                  deleting={deletingId === session.id}
                  copyLabel={uiText("复制恢复命令", "Copy resume command", "復元コマンドをコピー")}
                  copyTitle={uiText("复制恢复命令", "Copy resume command", "復元コマンドをコピー")}
                  deleteTitle={uiText("删除会话", "Delete session", "会話を削除")}
                  deleteLabel={uiText("删除会话", "Delete session", "会話を削除")}
                  unknownTimeLabel={uiText("未知时间", "Unknown time", "時刻不明")}
                  matchLabel={(count) => uiText(`${count} 处匹配`, `${count} match(es)`, `${count} 件一致`)}
                  itemsLabel={(count) => uiText(`${count} 条记录`, `${count} items`, `${count} 件`)}
                  onOpen={handleOpenSession}
                  onCopyResume={handleCopyResumeCommand}
                  onDelete={handleRequestDelete}
                />
              ))}
            </div>
          </div>

          <div className="section-card" style={{ display: "flex", flexDirection: "column", minHeight: 0 }}>
            {!selectedSession ? (
              <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-muted)", fontSize: 13 }}>
                {uiText("选择一个会话查看详情、目录导航和恢复入口", "Select a session to inspect details, TOC navigation, and restore actions", "会話を選択すると詳細・目次・復元操作を表示します")}
              </div>
            ) : detailLoading ? (
              <div className="loading-center" style={{ flex: 1 }}>
                <div className="spinner" />
                <span style={{ fontSize: 13, color: "var(--text-muted)" }}>
                  {uiText("正在读取会话详情...", "Loading session detail...", "会話の詳細を読み込み中...")}
                </span>
              </div>
            ) : detail ? (
              <>
                {/* Header: badges + title + delete */}
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 12, alignItems: "flex-start" }}>
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 6 }}>
                      <span className="badge badge-accent" style={{ fontSize: 10 }}>{detail.session.tool_name}</span>
                      <span className="badge badge-muted" style={{ fontSize: 10 }}>{detail.session.source_kind}</span>
                      {detail.session.created_at && <span className="badge badge-muted" style={{ fontSize: 10 }}>{detail.session.created_at}</span>}
                    </div>
                    <h3 style={{
                      fontSize: 15, fontWeight: 700, lineHeight: 1.35,
                      overflow: "hidden", textOverflow: "ellipsis",
                      display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical",
                    }}>
                      {detail.session.title}
                    </h3>
                  </div>
                  <button
                    className="btn btn-danger btn-xs"
                    onClick={() => setPendingDelete(detail.session)}
                    disabled={!detail.session.can_delete || deletingId === detail.session.id}
                    style={{ flexShrink: 0 }}
                  >
                    <Trash2 size={12} />
                    {uiText("删除", "Delete", "削除")}
                  </button>
                </div>

                {/* Resume command & directory — compact single bar */}
                <div style={{
                  display: "flex", flexDirection: "column", gap: 6, marginBottom: 12,
                  padding: "8px 12px", borderRadius: 6,
                  background: "var(--bg-elevated)", border: "1px solid var(--border-default)",
                  fontSize: 11, color: "var(--text-muted)",
                }}>
                  {buildResumeCommand(detail.session.tool_id, detail.session.id) && (
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <code style={{
                        fontFamily: "'JetBrains Mono', monospace", fontSize: 11,
                        flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                        userSelect: "all", cursor: "text",
                      }}>
                        {buildResumeCommand(detail.session.tool_id, detail.session.id)}
                      </code>
                      <button
                        className="btn btn-ghost btn-xs"
                        onClick={() => {
                          const cmd = buildResumeCommand(detail.session.tool_id, detail.session.id)!;
                          void navigator.clipboard.writeText(cmd).then(() =>
                            showToast("success", uiText("已复制恢复命令", "Resume command copied", "復元コマンドをコピーしました")),
                          );
                        }}
                        style={{ padding: "2px 5px", flexShrink: 0 }}
                      >
                        <Copy size={11} />
                      </button>
                    </div>
                  )}
                  {detail.session.cwd && (
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <FolderOpen size={11} style={{ flexShrink: 0, opacity: 0.6 }} />
                      <span style={{
                        flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                        userSelect: "all", cursor: "text",
                      }}>
                        {detail.session.cwd}
                      </span>
                      <button
                        className="btn btn-ghost btn-xs"
                        onClick={() => {
                          void navigator.clipboard.writeText(detail.session.cwd!).then(() =>
                            showToast("success", uiText("已复制目录路径", "Directory path copied", "ディレクトリパスをコピーしました")),
                          );
                        }}
                        style={{ padding: "2px 5px", flexShrink: 0 }}
                      >
                        <Copy size={11} />
                      </button>
                    </div>
                  )}
                </div>

                {/* Search bar */}
                <div style={{ display: "flex", gap: 10, alignItems: "center", marginBottom: 12 }}>
                  <div style={{ flex: 1, position: "relative" }}>
                    <Search size={13} style={{ position: "absolute", top: 10, left: 10, color: "var(--text-muted)" }} />
                    <input
                      className="input"
                      value={detailQuery}
                      onChange={(event) => setDetailQuery(event.target.value)}
                      placeholder={uiText("会话内搜索...", "Search within this session...", "この会話内を検索...")}
                      style={{ paddingLeft: 30, height: 34, fontSize: 12 }}
                    />
                  </div>
                  <span className="badge badge-muted" style={{ fontSize: 10, flexShrink: 0 }}>
                    {filteredEntries.length}
                  </span>
                </div>

                {/* Entries — full width, no TOC sidebar */}
                <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
                  {filteredEntries.length === 0 ? (
                    <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--text-muted)", fontSize: 13 }}>
                      {uiText("没有匹配的记录", "No entries matched", "一致する記録はありません")}
                    </div>
                  ) : (
                    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                      {filteredEntries.map((entry) => (
                        <section
                          key={entry.id}
                          id={`session-entry-${entry.id}`}
                          style={{
                            border: "1px solid var(--border-default)",
                            borderRadius: 8,
                            padding: 12,
                            background: "var(--bg-card)",
                          }}
                        >
                          <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
                            <div style={{ display: "flex", alignItems: "center", gap: 6, minWidth: 0 }}>
                              <span className={`badge ${entryBadgeColor(entry.kind)}`} style={{ fontSize: 10 }}>
                                {entry.kind}
                              </span>
                              <span style={{ fontSize: 12, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                {entry.title}
                              </span>
                            </div>
                            {entry.timestamp && (
                              <span style={{ fontSize: 10, color: "var(--text-muted)", flexShrink: 0 }}>{entry.timestamp}</span>
                            )}
                          </div>
                          <pre style={{
                            margin: 0, fontSize: 11, lineHeight: 1.5,
                            color: "var(--text-secondary)", whiteSpace: "pre-wrap", wordBreak: "break-word",
                            fontFamily: "'JetBrains Mono', monospace",
                            maxHeight: 320, overflow: "auto",
                          }}>
                            {entry.content}
                          </pre>
                        </section>
                      ))}
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-muted)", fontSize: 13 }}>
                {uiText("无法读取当前会话详情", "Failed to load this session detail", "この会話の詳細を読み込めませんでした")}
              </div>
            )}
          </div>
        </div>
      </div>

      <ConfirmDialog
        isOpen={Boolean(pendingDelete)}
        title={uiText("删除会话", "Delete Session", "会話を削除")}
        message={pendingDelete
          ? uiText(
            `确定删除会话「${pendingDelete.title}」吗？这会移除本地会话文件，并在支持的后端上清理索引。`,
            `Delete session "${pendingDelete.title}"? This removes the local session file and cleans indexes where supported.`,
            `会話「${pendingDelete.title}」を削除しますか？ ローカルの会話ファイルを削除し、対応バックエンドの索引も掃除します。`,
          )
          : ""}
        confirmText={uiText("删除", "Delete", "削除")}
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        onCancel={() => setPendingDelete(null)}
        onConfirm={() => void confirmDeleteSession()}
      />
    </>
  );
}
