import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Clock3,
  Database,
  FileText,
  FolderOpen,
  History,
  Play,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { getLocale } from "../lib/i18n";
import { fetchVisibleApps, getAppLabel, type ManagedAppId } from "../lib/appPreferences";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";

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

interface SessionResumeResult {
  launched: boolean;
  command: string;
  cwd: string | null;
}

const TOOL_ORDER: ManagedAppId[] = ["claude", "codex", "gemini", "opencode", "openclaw"];

function matchesEntry(entry: SessionEntry, query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  return (
    entry.title.toLowerCase().includes(normalized)
    || entry.kind.toLowerCase().includes(normalized)
    || entry.content.toLowerCase().includes(normalized)
  );
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
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedSession, setSelectedSession] = useState<SessionSummary | null>(null);
  const [detail, setDetail] = useState<SessionDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailQuery, setDetailQuery] = useState("");
  const [restoringId, setRestoringId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<SessionSummary | null>(null);
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const uiText = (zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  );

  useEffect(() => {
    void fetchVisibleApps().then(setVisibleApps).catch(() => setVisibleApps(TOOL_ORDER));
    void loadSessions(true);
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadSessions(false);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [filterTool, query]);

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
    if (showLoading) {
      setLoading(true);
    } else {
      setRefreshing(true);
    }
    try {
      const nextSessions = await invoke<SessionSummary[]>("get_sessions", {
        toolId: filterTool === "all" ? null : filterTool,
        query,
        limit: 240,
      });
      setSessions(nextSessions);

      if (selectedSession) {
        const nextSelected = nextSessions.find((item) => item.id === selectedSession.id && item.tool_id === selectedSession.tool_id) ?? null;
        setSelectedSession(nextSelected);
        if (nextSelected) {
          await openSession(nextSelected, false);
        } else {
          setDetail(null);
        }
      }
    } catch (error) {
      showToast("error", String(error));
      setSessions([]);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }

  async function openSession(session: SessionSummary, updateSelection = true) {
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
      setDetail(nextDetail);
      setDetailQuery("");
    } catch (error) {
      showToast("error", String(error));
      setDetail(null);
    } finally {
      setDetailLoading(false);
    }
  }

  async function handleResume(session: SessionSummary) {
    if (!session.can_resume) {
      showToast("error", uiText("当前会话来源暂不支持恢复", "This session backend cannot be resumed yet", "この会話バックエンドはまだ復元に対応していません"));
      return;
    }

    setRestoringId(session.id);
    try {
      const result = await invoke<SessionResumeResult>("resume_session_in_preferred_terminal", {
        toolId: session.tool_id,
        sessionId: session.id,
        cwd: session.cwd,
        sourcePath: session.source_path,
      });
      if (!result.launched && result.command) {
        try {
          await navigator.clipboard.writeText(result.command);
          showToast(
            "success",
            uiText(
              "已打开首选终端目录，恢复命令已复制到剪贴板",
              "Opened the preferred terminal directory and copied the resume command",
              "優先ターミナルのディレクトリを開き、復元コマンドをコピーしました",
            ),
          );
          return;
        } catch {
          showToast(
            "success",
            uiText(
              `已打开终端目录，请运行: ${result.command}`,
              `Opened the terminal directory. Run: ${result.command}`,
              `ターミナルのディレクトリを開きました。次を実行してください: ${result.command}`,
            ),
          );
          return;
        }
      }
      showToast(
        "success",
        uiText(
          "已在首选终端中发起会话恢复",
          "Started session restore in the preferred terminal",
          "優先ターミナルで会話の復元を開始しました",
        ),
      );
    } catch (error) {
      showToast("error", String(error));
    } finally {
      setRestoringId(null);
    }
  }

  async function confirmDeleteSession() {
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
  }

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

  const tocEntries = useMemo(
    () => filteredEntries.filter((entry) => entry.kind !== "tool_output" || entry.content.length < 8000),
    [filteredEntries],
  );

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
              ) : sessions.map((session) => {
                const selected = selectedSession?.id === session.id && selectedSession.tool_id === session.tool_id;
                return (
                  <button
                    key={`${session.tool_id}-${session.id}-${session.source_path}`}
                    className="card card-interactive"
                    onClick={() => void openSession(session)}
                    style={{
                      textAlign: "left",
                      padding: 14,
                      borderColor: selected ? "var(--accent)" : "var(--border-default)",
                      background: selected ? "var(--accent-subtle)" : "var(--bg-card)",
                    }}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start" }}>
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
                          <span className="badge badge-accent" style={{ fontSize: 10 }}>{session.tool_name}</span>
                          <span className="badge badge-muted" style={{ fontSize: 10 }}>{session.source_backend}</span>
                          {session.search_hit_count > 0 && query.trim() && (
                            <span className="badge badge-success" style={{ fontSize: 10 }}>
                              {uiText(`${session.search_hit_count} 处匹配`, `${session.search_hit_count} match(es)`, `${session.search_hit_count} 件一致`)}
                            </span>
                          )}
                        </div>
                        <div style={{ fontSize: 14, fontWeight: 700, color: "var(--text-primary)", lineHeight: 1.35 }}>
                          {session.title}
                        </div>
                        <div style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 6, lineHeight: 1.5 }}>
                          {session.preview}
                        </div>
                        <div style={{ display: "flex", gap: 12, flexWrap: "wrap", marginTop: 10, fontSize: 11, color: "var(--text-muted)" }}>
                          <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
                            <Clock3 size={12} />
                            {session.updated_at || session.created_at || uiText("未知时间", "Unknown time", "時刻不明")}
                          </span>
                          <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
                            <FileText size={12} />
                            {uiText(`${session.message_count} 条记录`, `${session.message_count} items`, `${session.message_count} 件`)}
                          </span>
                          {session.cwd && (
                            <span style={{ display: "inline-flex", alignItems: "center", gap: 5, minWidth: 0 }}>
                              <FolderOpen size={12} />
                              <span style={{ maxWidth: 180, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                {session.cwd}
                              </span>
                            </span>
                          )}
                        </div>
                      </div>
                      <div style={{ display: "flex", flexDirection: "column", gap: 8, flexShrink: 0 }}>
                        <button
                          className="btn btn-secondary btn-xs"
                          onClick={(event) => {
                            event.stopPropagation();
                            void handleResume(session);
                          }}
                          disabled={!session.can_resume || restoringId === session.id}
                          title={uiText("恢复会话", "Resume session", "会話を復元")}
                        >
                          <Play size={12} />
                        </button>
                        <button
                          className="btn btn-danger btn-xs"
                          onClick={(event) => {
                            event.stopPropagation();
                            setPendingDelete(session);
                          }}
                          disabled={!session.can_delete || deletingId === session.id}
                          title={uiText("删除会话", "Delete session", "会話を削除")}
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    </div>
                  </button>
                );
              })}
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
                <div style={{ display: "flex", justifyContent: "space-between", gap: 16, marginBottom: 14, flexWrap: "wrap" }}>
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 8 }}>
                      <span className="badge badge-accent" style={{ fontSize: 10 }}>{detail.session.tool_name}</span>
                      <span className="badge badge-muted" style={{ fontSize: 10 }}>{detail.session.source_kind}</span>
                      {detail.session.created_at && <span className="badge badge-muted" style={{ fontSize: 10 }}>{detail.session.created_at}</span>}
                    </div>
                    <h3 style={{ fontSize: 18, fontWeight: 700, lineHeight: 1.3 }}>{detail.session.title}</h3>
                    <div style={{ display: "flex", gap: 12, flexWrap: "wrap", marginTop: 8, fontSize: 12, color: "var(--text-muted)" }}>
                      {detail.session.cwd && (
                        <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
                          <FolderOpen size={13} />
                          {detail.session.cwd}
                        </span>
                      )}
                      <span style={{ display: "inline-flex", alignItems: "center", gap: 5 }}>
                        <Database size={13} />
                        {detail.session.source_path}
                      </span>
                    </div>
                  </div>
                  <div style={{ display: "flex", gap: 8, flexShrink: 0 }}>
                    <button
                      className="btn btn-secondary btn-sm"
                      onClick={() => void handleResume(detail.session)}
                      disabled={!detail.session.can_resume || restoringId === detail.session.id}
                    >
                      <Play size={14} />
                      {uiText("恢复", "Resume", "復元")}
                    </button>
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => setPendingDelete(detail.session)}
                      disabled={!detail.session.can_delete || deletingId === detail.session.id}
                    >
                      <Trash2 size={14} />
                      {uiText("删除", "Delete", "削除")}
                    </button>
                  </div>
                </div>

                <div style={{ display: "flex", gap: 12, alignItems: "center", marginBottom: 14, flexWrap: "wrap" }}>
                  <div style={{ flex: "1 1 280px", minWidth: 220, position: "relative" }}>
                    <Search size={14} style={{ position: "absolute", top: 11, left: 12, color: "var(--text-muted)" }} />
                    <input
                      className="input"
                      value={detailQuery}
                      onChange={(event) => setDetailQuery(event.target.value)}
                      placeholder={uiText("会话内搜索...", "Search within this session...", "この会話内を検索...")}
                      style={{ paddingLeft: 34 }}
                    />
                  </div>
                  <span className="badge badge-muted" style={{ fontSize: 10 }}>
                    {uiText(`${filteredEntries.length} 条可见记录`, `${filteredEntries.length} visible entries`, `${filteredEntries.length} 件を表示`)}
                  </span>
                </div>

                <div style={{ display: "grid", gridTemplateColumns: "minmax(220px, 260px) minmax(0, 1fr)", gap: 16, flex: 1, minHeight: 0 }}>
                  <div style={{ border: "1px solid var(--border-default)", borderRadius: 10, padding: 12, minHeight: 0, overflowY: "auto" }}>
                    <div style={{ fontSize: 12, fontWeight: 700, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em", marginBottom: 10 }}>
                      {uiText("目录导航", "TOC", "目次")}
                    </div>
                    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                      {tocEntries.length === 0 ? (
                        <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                          {uiText("没有匹配当前搜索的目录项", "No TOC entries match the current search", "現在の検索に一致する目次項目はありません")}
                        </div>
                      ) : tocEntries.map((entry, index) => (
                        <button
                          key={entry.id}
                          className="btn btn-ghost"
                          onClick={() => {
                            const target = document.getElementById(`session-entry-${entry.id}`);
                            target?.scrollIntoView({ behavior: "smooth", block: "start" });
                          }}
                          style={{ justifyContent: "flex-start", padding: "8px 10px", height: "auto", textAlign: "left" }}
                        >
                          <span style={{ display: "flex", flexDirection: "column", alignItems: "flex-start", gap: 4 }}>
                            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                              {index + 1}. {entry.timestamp || entry.kind}
                            </span>
                            <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", lineHeight: 1.4 }}>
                              {entry.title}
                            </span>
                          </span>
                        </button>
                      ))}
                    </div>
                  </div>

                  <div style={{ border: "1px solid var(--border-default)", borderRadius: 10, padding: 12, minHeight: 0, overflowY: "auto" }}>
                    {filteredEntries.length === 0 ? (
                      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--text-muted)", fontSize: 13 }}>
                        {uiText("没有匹配当前会话内搜索的记录", "No session entries matched the current search", "会話内検索に一致する記録はありません")}
                      </div>
                    ) : (
                      <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
                        {filteredEntries.map((entry) => (
                          <section
                            key={entry.id}
                            id={`session-entry-${entry.id}`}
                            style={{
                              border: "1px solid var(--border-default)",
                              borderRadius: 10,
                              padding: 14,
                              background: "var(--bg-card)",
                            }}
                          >
                            <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", marginBottom: 10, flexWrap: "wrap" }}>
                              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                                <span className={`badge ${entryBadgeColor(entry.kind)}`} style={{ fontSize: 10 }}>
                                  {entry.kind}
                                </span>
                                <span style={{ fontSize: 13, fontWeight: 700 }}>{entry.title}</span>
                              </div>
                              {entry.timestamp && (
                                <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{entry.timestamp}</span>
                              )}
                            </div>
                            <pre
                              style={{
                                margin: 0,
                                fontSize: 12,
                                lineHeight: 1.55,
                                color: "var(--text-secondary)",
                                whiteSpace: "pre-wrap",
                                wordBreak: "break-word",
                                fontFamily: "'JetBrains Mono', monospace",
                              }}
                            >
                              {entry.content}
                            </pre>
                          </section>
                        ))}
                      </div>
                    )}
                  </div>
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
