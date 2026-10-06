/* eslint-disable react-hooks/exhaustive-deps */
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { Copy, FolderOpen, History, RefreshCw, Search, SquareCheckBig, Trash2 } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { type ManagedAppId } from "../lib/appPreferences";
import { showToast } from "../components/Toast";
import ConfirmDialog from "../components/ConfirmDialog";
import SessionListItem from "../components/SessionListItem";
import SessionUsageActions from "../components/SessionUsageActions";
import LoadingState from "../components/states/LoadingState";
import ErrorState from "../components/states/ErrorState";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../components/ui/select";
import { useDeleteSessionMutation, useDeleteSessionsMutation } from "../hooks/mutations";
import { fetchVisibleAppsQuery, queryKeys } from "../hooks/queries";

import {
  buildResumeCommand,
  buildSessionToolFilters,
  buildSessionDeleteTarget,
  buildSessionListLabels,
  countSessionHits,
  matchesEntry,
  type SessionSummary,
  sameSession,
  sessionSelectionKey,
  deleteResultKey,
  TOOL_ORDER,
} from "./sessions/helpers";
import SessionEntries from "./sessions/Entries";
import { useSessionData } from "./sessions/useSessionData";
import RefreshError from "./sessions/RefreshError";
import DetailsHeader from "./sessions/DetailsHeader";
import TrashDialog from "./sessions/TrashDialog";
import DeleteFailures, { type DeleteFailure } from "./sessions/DeleteFailures";

export default function Sessions() {
  const queryClient = useQueryClient();
  const cachedVisibleApps = queryClient.getQueryData<ManagedAppId[]>(queryKeys.visibleApps);
  const [visibleApps, setVisibleApps] = useState<ManagedAppId[]>(cachedVisibleApps ?? TOOL_ORDER);
  const [filterTool, setFilterTool] = useState<ManagedAppId | "all">("all");
  const [query, setQuery] = useState("");
  const {
    allSessions,
    loading,
    refreshing,
    loadError,
    selectedSession,
    detail,
    detailLoading,
    detailError,
    detailQuery,
    setDetailQuery,
    loadSessions,
    openSession,
    closeSession,
    removeSessions,
  } = useSessionData(filterTool);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [bulkDeleting, setBulkDeleting] = useState(false);
  const bulkDeleteInFlight = useRef(false);
  const [checkedSessionKeys, setCheckedSessionKeys] = useState<string[]>([]);
  const [pendingDelete, setPendingDelete] = useState<SessionSummary | null>(null);
  const [pendingBulkDelete, setPendingBulkDelete] = useState<SessionSummary[]>([]);
  const [deleteFailures, setDeleteFailures] = useState<DeleteFailure[]>([]);
  const [trashOpen, setTrashOpen] = useState(false);
  const locale = getLocale();
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const deleteSessionMutation = useDeleteSessionMutation();
  const deleteSessionsMutation = useDeleteSessionsMutation();
  const deferredQuery = useDeferredValue(query);
  const uiText = (zhText: string, enText: string, jaText?: string) =>
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;

  // 把传给 SessionListItem 的 label/格式化函数缓存为稳定引用，避免每次渲染都让 memo 失效。
  const sessionListLabels = useMemo(() => buildSessionListLabels(locale), [locale]);

  useEffect(() => {
    void queryClient
      .fetchQuery({
        queryKey: queryKeys.visibleApps,
        queryFn: fetchVisibleAppsQuery,
      })
      .then(setVisibleApps)
      .catch(() => setVisibleApps(TOOL_ORDER));
  }, [queryClient]);

  useEffect(() => {
    const handleSearch = () => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    };
    const handleEscape = () => {
      if (bulkDeleteInFlight.current) return;
      if (pendingBulkDelete.length > 0) {
        setPendingBulkDelete([]);
        return;
      }
      if (pendingDelete) {
        setPendingDelete(null);
        return;
      }
      if (detailQuery) {
        setDetailQuery("");
        return;
      }
      if (selectedSession) {
        closeSession();
      }
    };
    window.addEventListener("cchub-shortcut-search", handleSearch);
    window.addEventListener("cchub-shortcut-escape", handleEscape);
    return () => {
      window.removeEventListener("cchub-shortcut-search", handleSearch);
      window.removeEventListener("cchub-shortcut-escape", handleEscape);
    };
  }, [closeSession, detailQuery, pendingBulkDelete.length, pendingDelete, selectedSession, setDetailQuery]);

  const deleteSingleSession = useCallback(
    async (session: SessionSummary) => {
      setDeletingId(sessionSelectionKey(session));
      try {
        await deleteSessionMutation.mutateAsync({
          toolId: session.tool_id,
          sessionId: session.id,
          sourcePath: session.source_path,
          sourceBackend: session.source_backend,
        });
        removeSessions([session]);
        setCheckedSessionKeys((current) => current.filter((key) => key !== sessionSelectionKey(session)));
        await loadSessions(false);
        showToast("success", uiText("会话已删除", "Session deleted", "会話を削除しました"));
      } catch (error) {
        showToast("error", String(error));
      } finally {
        setDeletingId(null);
      }
    },
    [deleteSessionMutation, loadSessions, removeSessions],
  );

  const confirmDeleteSession = useCallback(async () => {
    if (!pendingDelete) return;
    const target = pendingDelete;
    setPendingDelete(null);
    await deleteSingleSession(target);
  }, [deleteSingleSession, pendingDelete]);

  const confirmBulkDeleteSessions = useCallback(async () => {
    if (pendingBulkDelete.length === 0 || bulkDeleteInFlight.current) return;
    bulkDeleteInFlight.current = true;
    setBulkDeleting(true);
    try {
      const result = await deleteSessionsMutation.mutateAsync({
        sessions: pendingBulkDelete.map(buildSessionDeleteTarget),
      });
      const deletingKeys = new Set(result.deleted.map(deleteResultKey));
      removeSessions(pendingBulkDelete.filter((session) => deletingKeys.has(sessionSelectionKey(session))));
      setCheckedSessionKeys((current) => current.filter((key) => !deletingKeys.has(key)));
      setDeleteFailures(
        result.failed.flatMap(({ target, error }) => {
          const session = pendingBulkDelete.find(
            (candidate) => sessionSelectionKey(candidate) === deleteResultKey(target),
          );
          return session ? [{ session, error }] : [];
        }),
      );
      setPendingBulkDelete([]);
      await loadSessions(false);
      if (result.failed.length)
        showToast(
          "error",
          uiText(
            `${result.failed.length} 个会话未删除，可重试失败项`,
            `${result.failed.length} sessions could not be deleted; retry failed items`,
            `${result.failed.length} 件を削除できませんでした。失敗した項目を再試行できます`,
          ),
        );
      else showToast("success", uiText("已删除选中的会话", "Selected sessions deleted", "選択した会話を削除しました"));
    } catch (error) {
      showToast("error", String(error));
    } finally {
      bulkDeleteInFlight.current = false;
      setBulkDeleting(false);
    }
  }, [deleteSessionsMutation, loadSessions, pendingBulkDelete, removeSessions]);

  const handleOpenSession = useCallback(
    (session: SessionSummary) => {
      void openSession(session);
    },
    [openSession],
  );

  const handleCopyResumeCommand = useCallback(
    (command: string) => {
      void navigator.clipboard
        .writeText(command)
        .then(() =>
          showToast("success", uiText("已复制恢复命令", "Resume command copied", "復元コマンドをコピーしました")),
        )
        .catch(() =>
          showToast(
            "error",
            uiText("复制失败，请重试", "Copy failed; please retry", "コピーに失敗しました。再試行してください"),
          ),
        );
    },
    [uiText],
  );

  const handleRequestDelete = useCallback((session: SessionSummary) => {
    setPendingDelete(session);
  }, []);

  const toolFilters = useMemo<Array<{ id: ManagedAppId | "all"; label: string }>>(
    () => buildSessionToolFilters(visibleApps, uiText("全部 App", "All Apps", "すべての App")),
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
  const checkedSessionKeySet = useMemo(() => new Set(checkedSessionKeys), [checkedSessionKeys]);
  const checkedSessions = useMemo(
    () => sessions.filter((session) => session.can_delete && checkedSessionKeySet.has(sessionSelectionKey(session))),
    [checkedSessionKeySet, sessions],
  );

  useEffect(() => {
    const visibleKeys = new Set(allSessions.map(sessionSelectionKey));
    setCheckedSessionKeys((current) => current.filter((key) => visibleKeys.has(key)));
  }, [allSessions]);

  const handleToggleCheckedSession = useCallback((session: SessionSummary) => {
    if (!session.can_delete) return;
    const key = sessionSelectionKey(session);
    setCheckedSessionKeys((current) =>
      current.includes(key) ? current.filter((item) => item !== key) : [...current, key],
    );
  }, []);

  const handleToggleAllVisibleSessions = useCallback(() => {
    const visibleKeys = sessions.filter((session) => session.can_delete).map(sessionSelectionKey);
    if (visibleKeys.length === 0) return;
    const allChecked = visibleKeys.every((key) => checkedSessionKeySet.has(key));
    setCheckedSessionKeys((current) => {
      if (allChecked) {
        return current.filter((key) => !visibleKeys.includes(key));
      }
      const next = new Set(current);
      visibleKeys.forEach((key) => next.add(key));
      return [...next];
    });
  }, [checkedSessionKeySet, sessions]);

  const handleRequestBulkDelete = useCallback(() => {
    if (checkedSessions.length === 0) return;
    setPendingBulkDelete(checkedSessions);
  }, [checkedSessions]);

  if (loading) {
    return <LoadingState label={uiText("加载会话中...", "Loading sessions...", "会話を読み込み中...")} />;
  }
  if (loadError && allSessions.length === 0) {
    return (
      <ErrorState
        title={uiText("无法加载会话", "Unable to load sessions", "会話を読み込めません")}
        message={loadError}
        retryLabel={uiText("重试", "Retry", "再試行")}
        onRetry={() => void loadSessions(true)}
      />
    );
  }

  return (
    <>
      <div style={{ height: "100%", display: "flex", flexDirection: "column", gap: 16 }}>
        <div className="page-header" style={{ marginBottom: 0 }}>
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
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <SessionUsageActions />
            <Button variant="secondary" onClick={() => setTrashOpen(true)}>
              <History size={14} />
              {uiText("最近删除", "Recently deleted", "最近削除した会話")}
            </Button>
            <Button type="button" variant="secondary" onClick={() => void loadSessions(false)}>
              <RefreshCw size={14} className={refreshing ? "spin" : undefined} />
              {uiText("刷新", "Refresh", "更新")}
            </Button>
          </div>
        </div>

        <DeleteFailures
          failures={deleteFailures}
          busy={bulkDeleting}
          onRetry={() => setPendingBulkDelete(deleteFailures.map(({ session }) => session))}
          onDismiss={() => setDeleteFailures([])}
          uiText={uiText}
        />
        <Card className="section-card" style={{ padding: 16 }}>
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
            <div style={{ flex: "1 1 320px", minWidth: 240, position: "relative" }}>
              <Search
                size={14}
                style={{
                  position: "absolute",
                  top: "50%",
                  transform: "translateY(-50%)",
                  left: 12,
                  color: "var(--text-muted)",
                }}
              />
              <Input
                ref={searchInputRef}
                aria-label={uiText("搜索会话", "Search sessions", "会話を検索")}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={uiText(
                  "搜索标题、路径或摘要...",
                  "Search title, path, or preview...",
                  "タイトル・パス・要約を検索...",
                )}
                style={{ paddingLeft: 34 }}
              />
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <Select value={filterTool} onValueChange={(value) => setFilterTool(value as ManagedAppId | "all")}>
                <SelectTrigger aria-label={uiText("筛选 App", "Filter app", "App を絞り込む")} style={{ width: 160 }}>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {toolFilters.map((option) => (
                    <SelectItem key={option.id} value={option.id}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <Button
                type="button"
                variant="secondary"
                onClick={handleToggleAllVisibleSessions}
                disabled={!sessions.some((session) => session.can_delete)}
              >
                <SquareCheckBig size={14} />
                {checkedSessions.length === sessions.filter((session) => session.can_delete).length &&
                checkedSessions.length > 0
                  ? uiText("清空当前选择", "Clear visible selection", "現在の選択を解除")
                  : uiText("全选当前结果", "Select visible results", "現在の結果を全選択")}
              </Button>
              <Button
                type="button"
                variant="destructive"
                onClick={handleRequestBulkDelete}
                disabled={checkedSessions.length === 0 || bulkDeleting}
              >
                <Trash2 size={14} />
                {uiText(
                  `删除选中 (${checkedSessions.length})`,
                  `Delete selected (${checkedSessions.length})`,
                  `選択を削除 (${checkedSessions.length})`,
                )}
              </Button>
            </div>
          </div>
        </Card>

        {loadError && (
          <RefreshError
            title={uiText(
              "刷新失败，已保留当前会话",
              "Refresh failed; current sessions were kept",
              "更新に失敗しました。現在の会話を保持しています",
            )}
            message={loadError}
            retryLabel={uiText("重试", "Retry", "再試行")}
            onRetry={() => void loadSessions(false)}
          />
        )}

        <div className={`sessions-workspace ${selectedSession ? "sessions-detail-open" : ""}`}>
          <Card
            className="section-card sessions-list-card"
            style={{ display: "flex", flexDirection: "column", minHeight: 0 }}
          >
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <History size={15} style={{ color: "var(--text-secondary)" }} />
                <span
                  style={{
                    fontSize: 12,
                    fontWeight: 600,
                    color: "var(--text-muted)",
                    textTransform: "uppercase",
                    letterSpacing: "0.05em",
                  }}
                >
                  {uiText("会话列表", "Session List", "会話一覧")}
                </span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                {checkedSessions.length > 0 && (
                  <span className="badge badge-accent" style={{ fontSize: 11 }}>
                    {uiText(
                      `已选 ${checkedSessions.length}`,
                      `${checkedSessions.length} selected`,
                      `${checkedSessions.length} 件選択`,
                    )}
                  </span>
                )}
                <span className="badge badge-muted" style={{ fontSize: 11 }}>
                  {sessions.length}
                </span>
              </div>
            </div>

            <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
              {sessions.length === 0 ? (
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    flex: 1,
                    color: "var(--text-muted)",
                    fontSize: 14,
                  }}
                >
                  {uiText(
                    "当前没有匹配的会话",
                    "No sessions matched the current filters",
                    "現在の条件に一致する会話はありません",
                  )}
                </div>
              ) : (
                sessions.map((session) => (
                  <SessionListItem
                    key={sessionSelectionKey(session)}
                    session={session}
                    selected={sameSession(selectedSession, session)}
                    query={query}
                    resumeCommand={buildResumeCommand(session.tool_id, session.id)}
                    deleting={deletingId === sessionSelectionKey(session)}
                    checked={checkedSessionKeySet.has(sessionSelectionKey(session))}
                    copyLabel={sessionListLabels.copyLabel}
                    copyTitle={sessionListLabels.copyTitle}
                    deleteTitle={sessionListLabels.deleteTitle}
                    deleteLabel={sessionListLabels.deleteLabel}
                    selectLabel={sessionListLabels.selectLabel}
                    tokenLabel={sessionListLabels.tokenLabel}
                    unknownTimeLabel={sessionListLabels.unknownTimeLabel}
                    matchLabel={sessionListLabels.matchLabel}
                    itemsLabel={sessionListLabels.itemsLabel}
                    onOpen={handleOpenSession}
                    onToggleChecked={handleToggleCheckedSession}
                    onCopyResume={handleCopyResumeCommand}
                    onDelete={handleRequestDelete}
                  />
                ))
              )}
            </div>
          </Card>

          <Card
            className="section-card sessions-detail-card"
            style={{ display: "flex", flexDirection: "column", minHeight: 0 }}
          >
            {selectedSession && (
              <DetailsHeader
                session={selectedSession}
                query={detailQuery}
                locale={locale}
                deleting={deletingId === sessionSelectionKey(selectedSession)}
                onDelete={() => setPendingDelete(selectedSession)}
                onClose={closeSession}
              />
            )}
            {!selectedSession ? (
              <div
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "var(--text-muted)",
                  fontSize: 14,
                }}
              >
                {uiText(
                  "选择一个会话查看详情、目录导航和恢复入口",
                  "Select a session to inspect details, TOC navigation, and restore actions",
                  "会話を選択すると詳細・目次・復元操作を表示します",
                )}
              </div>
            ) : detailLoading ? (
              <LoadingState
                label={uiText("正在读取会话详情...", "Loading session detail...", "会話の詳細を読み込み中...")}
              />
            ) : detailError ? (
              <ErrorState
                title={uiText("无法读取会话详情", "Unable to load session details", "会話の詳細を読み込めません")}
                message={detailError}
                retryLabel={uiText("重试", "Retry", "再試行")}
                onRetry={() => void openSession(selectedSession, false)}
              />
            ) : detail ? (
              <>
                {/* Resume command & directory — compact single bar */}
                <div
                  style={{
                    display: "flex",
                    flexDirection: "column",
                    gap: 6,
                    marginBottom: 12,
                    padding: "8px 12px",
                    borderRadius: 6,
                    background: "var(--bg-elevated)",
                    border: "1px solid var(--border-default)",
                    fontSize: 11,
                    color: "var(--text-muted)",
                  }}
                >
                  {buildResumeCommand(detail.session.tool_id, detail.session.id) && (
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <code
                        style={{
                          fontFamily: "var(--font-code)",
                          fontSize: 11,
                          flex: 1,
                          minWidth: 0,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          userSelect: "all",
                          cursor: "text",
                        }}
                      >
                        {buildResumeCommand(detail.session.tool_id, detail.session.id)}
                      </code>
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        aria-label={sessionListLabels.copyLabel}
                        onClick={() =>
                          handleCopyResumeCommand(buildResumeCommand(detail.session.tool_id, detail.session.id)!)
                        }
                      >
                        <Copy size={12} />
                      </Button>
                    </div>
                  )}
                  {detail.session.cwd && (
                    <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                      <FolderOpen size={11} style={{ flexShrink: 0, opacity: 0.6 }} />
                      <span
                        style={{
                          flex: 1,
                          minWidth: 0,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          userSelect: "all",
                          cursor: "text",
                        }}
                      >
                        {detail.session.cwd}
                      </span>
                      <Button
                        variant="ghost"
                        size="icon-xs"
                        aria-label={uiText("复制目录路径", "Copy directory path", "ディレクトリパスをコピー")}
                        onClick={() => {
                          void navigator.clipboard
                            .writeText(detail.session.cwd!)
                            .then(() =>
                              showToast(
                                "success",
                                uiText("已复制目录路径", "Directory path copied", "ディレクトリパスをコピーしました"),
                              ),
                            )
                            .catch(() =>
                              showToast(
                                "error",
                                uiText(
                                  "复制失败，请重试",
                                  "Copy failed; please retry",
                                  "コピーに失敗しました。再試行してください",
                                ),
                              ),
                            );
                        }}
                      >
                        <Copy size={12} />
                      </Button>
                    </div>
                  )}
                </div>

                {/* Search bar */}
                <div style={{ display: "flex", gap: 10, alignItems: "center", marginBottom: 12 }}>
                  <div style={{ flex: 1, position: "relative" }}>
                    <Search
                      size={14}
                      style={{
                        position: "absolute",
                        top: "50%",
                        transform: "translateY(-50%)",
                        left: 10,
                        color: "var(--text-muted)",
                      }}
                    />
                    <Input
                      aria-label={uiText("会话内搜索", "Search within session", "会話内を検索")}
                      value={detailQuery}
                      onChange={(event) => setDetailQuery(event.target.value)}
                      placeholder={uiText("会话内搜索...", "Search within this session...", "この会話内を検索...")}
                      style={{ paddingLeft: 30 }}
                    />
                  </div>
                  <span className="badge badge-muted" style={{ fontSize: 11, flexShrink: 0 }}>
                    {filteredEntries.length}
                  </span>
                </div>

                <SessionEntries
                  entries={filteredEntries}
                  query={detailQuery}
                  emptyLabel={uiText("没有匹配的记录", "No entries matched", "一致する記録はありません")}
                />
              </>
            ) : (
              <div
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "var(--text-muted)",
                  fontSize: 14,
                }}
              >
                {uiText(
                  "无法读取当前会话详情",
                  "Failed to load this session detail",
                  "この会話の詳細を読み込めませんでした",
                )}
              </div>
            )}
          </Card>
        </div>
      </div>

      <TrashDialog
        open={trashOpen}
        onClose={() => setTrashOpen(false)}
        onRestored={() => void loadSessions(false)}
        uiText={uiText}
      />
      <ConfirmDialog
        isOpen={Boolean(pendingDelete)}
        title={uiText("删除会话", "Delete Session", "会話を削除")}
        message={
          pendingDelete
            ? uiText(
                `确定删除会话「${pendingDelete.title}」吗？Codex 会话可在“最近删除”中恢复，其他 App 的会话无法在这里恢复。`,
                `Delete session "${pendingDelete.title}"? Codex sessions can be restored from Recently deleted. Sessions from other apps cannot be restored here.`,
                `会話「${pendingDelete.title}」を削除しますか？ Codex の会話は「最近削除した会話」で復元できます。他の App の会話はここでは復元できません。`,
              )
            : ""
        }
        confirmText={uiText("删除", "Delete", "削除")}
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        onCancel={() => setPendingDelete(null)}
        onConfirm={() => void confirmDeleteSession()}
      />

      <ConfirmDialog
        isOpen={pendingBulkDelete.length > 0}
        title={uiText("批量删除会话", "Delete Selected Sessions", "選択した会話を削除")}
        message={
          pendingBulkDelete.length > 0
            ? uiText(
                `确定删除选中的 ${pendingBulkDelete.length} 个会话吗？Codex 会话可在“最近删除”中恢复，其他 App 的会话无法在这里恢复。`,
                `Delete ${pendingBulkDelete.length} selected session(s)? Codex sessions can be restored from Recently deleted. Sessions from other apps cannot be restored here.`,
                `選択した ${pendingBulkDelete.length} 件の会話を削除しますか？ Codex の会話は「最近削除した会話」で復元できます。他の App の会話はここでは復元できません。`,
              )
            : ""
        }
        confirmText={
          bulkDeleting ? uiText("删除中…", "Deleting…", "削除中…") : uiText("批量删除", "Delete Selected", "選択を削除")
        }
        busy={bulkDeleting}
        cancelText={uiText("取消", "Cancel", "キャンセル")}
        variant="destructive"
        onCancel={() => setPendingBulkDelete([])}
        onConfirm={() => void confirmBulkDeleteSessions()}
      />
    </>
  );
}
