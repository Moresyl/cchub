import { memo, useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, FileText, Globe, Package, Wrench } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";
import {
  buildProviderProfileFromDeepLink,
  decodeDeepLinkText,
  deeplinkApi,
  getPrimaryDeepLinkEndpoint,
  maskSecret,
  parseMcpPreviewServers,
  splitDeepLinkEndpoints,
  type DeepLinkErrorPayload,
  type DeepLinkImportRequest,
  type DeepLinkMcpImportResult,
} from "../lib/deeplink";

interface PromptPreset {
  id: string;
  name: string;
  content: string;
}

interface SkillRegistryEntry {
  id: string;
  name: string;
  description: string;
  description_zh: string | null;
  category: string;
  author: string | null;
  github_url: string | null;
  cover_url: string | null;
  tags: string[];
  content: string;
}

interface ProviderPreviewSectionProps {
  current: DeepLinkImportRequest;
  unnamedLabel: string;
  primaryEndpointLabel: string;
  endpointCandidatesLabel: string;
  homepageLabel: string;
}

interface PromptPreviewSectionProps {
  current: DeepLinkImportRequest;
  unnamedLabel: string;
  contentPreviewLabel: string;
  emptyContentLabel: string;
}

interface McpPreviewSectionProps {
  current: DeepLinkImportRequest;
  unavailablePreviewLabel: string;
}

interface SkillPreviewSectionProps {
  current: DeepLinkImportRequest;
  fetchDescription: string;
}

function ProviderPreviewSectionComponent({
  current,
  unnamedLabel,
  primaryEndpointLabel,
  endpointCandidatesLabel,
  homepageLabel,
}: ProviderPreviewSectionProps) {
  const endpoints = useMemo(() => splitDeepLinkEndpoints(current.endpoint), [current.endpoint]);
  const primaryEndpoint = useMemo(() => getPrimaryDeepLinkEndpoint(current), [current]);

  return (
    <section className="section-card" style={{ padding: 14 }}>
      <div className="field-label">Provider</div>
      <div style={{ display: "grid", gap: 10 }}>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          <span className="badge badge-accent">{current.app}</span>
          <span className="badge badge-muted">{current.name || unnamedLabel}</span>
        </div>
        {primaryEndpoint && (
          <div>
            <div className="field-label">{primaryEndpointLabel}</div>
            <div style={{ fontSize: 13, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
              {primaryEndpoint}
            </div>
          </div>
        )}
        {endpoints.length > 1 && (
          <div>
            <div className="field-label">{endpointCandidatesLabel}</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {endpoints.map((endpoint) => (
                <div key={endpoint} style={{ fontSize: 12, color: "var(--text-secondary)", wordBreak: "break-all" }}>
                  {endpoint}
                </div>
              ))}
            </div>
          </div>
        )}
        {(current.model || current.apiFormat) && (
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}>
            {current.model && <span className="badge badge-muted">{`Model: ${current.model}`}</span>}
            {current.apiFormat && <span className="badge badge-muted">{`API: ${current.apiFormat}`}</span>}
          </div>
        )}
        {current.apiKey && (
          <div>
            <div className="field-label">API Key</div>
            <div style={{ fontSize: 13, fontFamily: "'JetBrains Mono', monospace" }}>
              {maskSecret(current.apiKey)}
            </div>
          </div>
        )}
        {current.homepage && (
          <div>
            <div className="field-label">{homepageLabel}</div>
            <div style={{ fontSize: 13, color: "var(--text-secondary)", wordBreak: "break-all" }}>
              {current.homepage}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

const ProviderPreviewSection = memo(ProviderPreviewSectionComponent);

function PromptPreviewSectionComponent({
  current,
  unnamedLabel,
  contentPreviewLabel,
  emptyContentLabel,
}: PromptPreviewSectionProps) {
  const decodedContent = useMemo(() => decodeDeepLinkText(current.content), [current.content]);

  return (
    <section className="section-card" style={{ padding: 14 }}>
      <div className="field-label">Prompt</div>
      <div style={{ display: "grid", gap: 10 }}>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          {current.app && <span className="badge badge-accent">{current.app}</span>}
          <span className="badge badge-muted">{current.name || unnamedLabel}</span>
        </div>
        {current.description && (
          <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>{current.description}</div>
        )}
        <div>
          <div className="field-label">{contentPreviewLabel}</div>
          <div
            style={{
              padding: "12px 14px",
              borderRadius: 8,
              background: "var(--bg-input)",
              border: "1px solid var(--border-default)",
              fontSize: 13,
              lineHeight: 1.6,
              maxHeight: 240,
              overflow: "auto",
              whiteSpace: "pre-wrap",
            }}
          >
            {decodedContent || emptyContentLabel}
          </div>
        </div>
      </div>
    </section>
  );
}

const PromptPreviewSection = memo(PromptPreviewSectionComponent);

function McpPreviewSectionComponent({
  current,
  unavailablePreviewLabel,
}: McpPreviewSectionProps) {
  const mcpServers = useMemo(() => parseMcpPreviewServers(current), [current]);
  const appBadges = useMemo(
    () => (current.apps || "").split(",").map((app) => app.trim()).filter(Boolean),
    [current.apps],
  );

  return (
    <section className="section-card" style={{ padding: 14 }}>
      <div className="field-label">MCP</div>
      <div style={{ display: "grid", gap: 12 }}>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {appBadges.map((app) => (
            <span key={app} className="badge badge-accent">{app}</span>
          ))}
        </div>
        {mcpServers.length > 0 ? (
          <div style={{ display: "grid", gap: 10 }}>
            {mcpServers.map((server) => (
              <div key={server.name} style={{ padding: "12px 14px", borderRadius: 8, background: "var(--bg-input)", border: "1px solid var(--border-default)" }}>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
                  <strong style={{ fontSize: 13 }}>{server.name}</strong>
                  <span className="badge badge-muted">{server.transport}</span>
                </div>
                <div style={{ marginTop: 6, fontSize: 12, fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
                  {server.command}{server.args.length ? ` ${server.args.join(" ")}` : ""}
                </div>
                {server.envKeys.length > 0 && (
                  <div style={{ marginTop: 8, display: "flex", gap: 6, flexWrap: "wrap" }}>
                    {server.envKeys.map((key) => (
                      <span key={key} className="badge badge-muted">{key}</span>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : (
          <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>
            {unavailablePreviewLabel}
          </div>
        )}
      </div>
    </section>
  );
}

const McpPreviewSection = memo(McpPreviewSectionComponent);

function SkillPreviewSectionComponent({
  current,
  fetchDescription,
}: SkillPreviewSectionProps) {
  return (
    <section className="section-card" style={{ padding: 14 }}>
      <div className="field-label">Skill</div>
      <div style={{ display: "grid", gap: 10 }}>
        {current.name && <span className="badge badge-muted">{current.name}</span>}
        {current.repo && (
          <div>
            <div className="field-label">Repository</div>
            <div style={{ fontSize: 13, fontFamily: "'JetBrains Mono', monospace" }}>{current.repo}</div>
          </div>
        )}
        {(current.branch || current.directory) && (
          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            {current.branch && <span className="badge badge-muted">{`Branch: ${current.branch}`}</span>}
            {current.directory && <span className="badge badge-muted">{`Dir: ${current.directory}`}</span>}
          </div>
        )}
        <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>
          {fetchDescription}
        </div>
      </div>
    </section>
  );
}

const SkillPreviewSection = memo(SkillPreviewSectionComponent);

function requestFingerprint(request: DeepLinkImportRequest) {
  return JSON.stringify(request);
}

function normalizeDirectory(value: string | undefined) {
  return (value || "").replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
}

function DeepLinkImportDialogComponent() {
  const [queue, setQueue] = useState<DeepLinkImportRequest[]>([]);
  const [importing, setImporting] = useState(false);
  const locale = getLocale();
  const current = queue[0] || null;

  const uiText = useCallback((zhText: string, enText: string, jaText?: string) => (
    locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText
  ), [locale]);

  const enqueueRequest = useCallback(async (request: DeepLinkImportRequest, disposed = false) => {
    try {
      const prepared = request.resource === "provider" && (request.config || request.configUrl)
        ? await deeplinkApi.mergeRequest(request)
        : request;
      if (disposed) return;
      setQueue((currentQueue) => (
        currentQueue.some((item) => requestFingerprint(item) === requestFingerprint(prepared))
          ? currentQueue
          : [...currentQueue, prepared]
      ));
    } catch (error) {
      if (disposed) return;
      showToast(
        "error",
        uiText(
          `Deep Link 解析失败: ${error}`,
          `Failed to prepare deep link: ${error}`,
          `Deep Link の準備に失敗しました: ${error}`,
        ),
      );
    }
  }, [uiText]);

  const loadPending = useCallback(async (disposed = false) => {
    try {
      const [pendingErrors, pendingImports] = await Promise.all([
        deeplinkApi.takePendingErrors(),
        deeplinkApi.takePendingImports(),
      ]);
      if (disposed) return;
      for (const item of pendingErrors) {
        showToast("error", item.error);
      }
      for (const item of pendingImports) {
        // eslint-disable-next-line no-await-in-loop
        await enqueueRequest(item, disposed);
      }
    } catch (error) {
      if (!disposed) {
        showToast(
          "error",
          uiText(
            `读取待处理 Deep Link 失败: ${error}`,
            `Failed to load pending deep links: ${error}`,
            `保留中の Deep Link 読み込みに失敗しました: ${error}`,
          ),
        );
      }
    }
  }, [enqueueRequest, uiText]);

  useEffect(() => {
    let disposed = false;

    const importListener = listen<DeepLinkImportRequest>("deeplink-import", (event) => {
      void enqueueRequest(event.payload, disposed);
    });
    const errorListener = listen<DeepLinkErrorPayload>("deeplink-error", (event) => {
      showToast("error", event.payload.error);
    });

    void loadPending(disposed);

    return () => {
      disposed = true;
      importListener.then((unlisten) => unlisten());
      errorListener.then((unlisten) => unlisten());
    };
  }, [enqueueRequest, loadPending]);

  const resolveSkill = useCallback(async (request: DeepLinkImportRequest): Promise<SkillRegistryEntry> => {
    const repo = request.repo?.trim();
    if (!repo) {
      throw new Error(uiText("技能 Deep Link 缺少 repo 参数", "Skill deep link is missing repo", "Skill Deep Link に repo がありません"));
    }

    const [owner, repoName] = repo.split("/");
    const branch = request.branch?.trim() || "main";
    const entries = await invoke<SkillRegistryEntry[]>("fetch_skills_from_repo", {
      owner,
      repo: repoName,
      branch,
    });

    const targetDirectory = normalizeDirectory(request.directory);
    if (targetDirectory) {
      const matchedByDirectory = entries.find((entry) => {
        const entryDirectory = entry.id.split(":").slice(1).join(":");
        return normalizeDirectory(entryDirectory) === targetDirectory;
      });
      if (matchedByDirectory) return matchedByDirectory;
    }

    const targetName = request.name?.trim().toLowerCase();
    if (targetName) {
      const matchedByName = entries.find((entry) => entry.name.trim().toLowerCase() === targetName);
      if (matchedByName) return matchedByName;
    }

    if (entries.length === 1) {
      return entries[0];
    }

    throw new Error(uiText(
      "技能仓库包含多个技能，请在 Deep Link 中补充 directory 参数",
      "Skill repository contains multiple skills. Add a directory parameter to the deep link.",
      "Skill リポジトリに複数のスキルがあります。Deep Link に directory を指定してください。",
    ));
  }, [uiText]);

  const handleConfirm = useCallback(async () => {
    if (!current || importing) return;
    setImporting(true);
    try {
      if (current.resource === "provider") {
        const profile = buildProviderProfileFromDeepLink(current);
        const profileId = await invoke<string>("save_config_profile", {
          name: profile.name,
          toolId: profile.toolId,
          configSnapshot: profile.configSnapshot,
        });
        if (current.enabled) {
          await invoke("apply_config_profile", { id: profileId });
        }
        await invoke("refresh_tray_provider_menu");
        showToast(
          "success",
          uiText(
            `Provider 已导入: ${profile.name}`,
            `Provider imported: ${profile.name}`,
            `Provider をインポートしました: ${profile.name}`,
          ),
        );
      } else if (current.resource === "prompt") {
        const preset = await invoke<PromptPreset>("save_prompt_preset", {
          id: null,
          name: current.name?.trim() || uiText("导入提示词", "Imported Prompt", "インポートした Prompt"),
          content: decodeDeepLinkText(current.content),
        });
        if (current.enabled) {
          await invoke("activate_prompt_preset", { id: preset.id });
        }
        showToast(
          "success",
          uiText(
            `提示词已导入: ${preset.name}`,
            `Prompt imported: ${preset.name}`,
            `Prompt をインポートしました: ${preset.name}`,
          ),
        );
      } else if (current.resource === "mcp") {
        const result = await deeplinkApi.importMcp(current);
        await invoke("scan_mcp_servers");
        const typedResult = result as DeepLinkMcpImportResult;
        if (typedResult.failed.length > 0) {
          showToast(
            "info",
            uiText(
              `MCP 已导入 ${typedResult.importedCount} 个，失败 ${typedResult.failed.length} 个`,
              `Imported ${typedResult.importedCount} MCP server(s), failed ${typedResult.failed.length}`,
              `${typedResult.importedCount} 件の MCP を導入し、${typedResult.failed.length} 件失敗しました`,
            ),
          );
        } else {
          showToast(
            "success",
            uiText(
              `MCP 已导入 ${typedResult.importedCount} 个`,
              `Imported ${typedResult.importedCount} MCP server(s)`,
              `${typedResult.importedCount} 件の MCP をインポートしました`,
            ),
          );
        }
      } else if (current.resource === "skill") {
        const skill = await resolveSkill(current);
        await invoke<string>("install_skill_from_marketplace", {
          name: skill.name,
          content: skill.content,
          targetDir: null,
        });
        showToast(
          "success",
          uiText(
            `技能已导入: ${skill.name}`,
            `Skill imported: ${skill.name}`,
            `Skill をインポートしました: ${skill.name}`,
          ),
        );
      } else {
        throw new Error(uiText("暂不支持的 Deep Link 类型", "Unsupported deep link type", "未対応の Deep Link 種別です"));
      }

      setQueue((currentQueue) => currentQueue.slice(1));
    } catch (error) {
      showToast(
        "error",
        uiText(
          `导入失败: ${error}`,
          `Import failed: ${error}`,
          `インポートに失敗しました: ${error}`,
        ),
      );
    } finally {
      setImporting(false);
    }
  }, [current, importing, resolveSkill, uiText]);

  const handleCancel = useCallback(() => {
    if (importing) return;
    setQueue((currentQueue) => currentQueue.slice(1));
  }, [importing]);

  if (!current) return null;

  const resourceTitle = useMemo(() => (
    current.resource === "provider"
      ? uiText("导入 Provider", "Import Provider", "Provider をインポート")
      : current.resource === "prompt"
        ? uiText("导入提示词", "Import Prompt", "Prompt をインポート")
        : current.resource === "mcp"
          ? uiText("导入 MCP", "Import MCP", "MCP をインポート")
          : uiText("导入技能", "Import Skill", "Skill をインポート")
  ), [current.resource, uiText]);
  const ResourceIcon = useMemo(() => (
    current.resource === "provider"
      ? Globe
      : current.resource === "prompt"
        ? FileText
        : current.resource === "mcp"
          ? Wrench
          : Package
  ), [current.resource]);
  const providerUnnamedLabel = uiText("未命名 Provider", "Unnamed Provider", "無名の Provider");
  const promptUnnamedLabel = uiText("未命名提示词", "Unnamed Prompt", "無名の Prompt");
  const primaryEndpointLabel = uiText("主端点", "Primary Endpoint", "プライマリエンドポイント");
  const endpointCandidatesLabel = uiText("候选端点", "Endpoint Candidates", "候補エンドポイント");
  const homepageLabel = uiText("主页", "Homepage", "ホームページ");
  const contentPreviewLabel = uiText("内容预览", "Content Preview", "内容プレビュー");
  const emptyContentLabel = uiText("无内容", "No content", "内容なし");
  const unavailablePreviewLabel = uiText("无法预览 MCP 配置，将在导入时校验。", "MCP config preview is unavailable and will be validated on import.", "MCP 設定はプレビューできません。インポート時に検証します。");
  const skillFetchDescription = uiText(
    "确认时将从远程仓库拉取技能内容并安装到当前技能目录。",
    "The skill content will be fetched from the repository and installed into the current skills directory on confirmation.",
    "確認時にリポジトリから Skill を取得し、現在の Skill ディレクトリへインストールします。",
  );

  return (
    <div className="confirm-overlay" onClick={handleCancel}>
      <div
        className="confirm-dialog animate-in"
        onClick={(event) => event.stopPropagation()}
        style={{ maxWidth: 720, padding: 22 }}
      >
        <div style={{ display: "flex", gap: 14, alignItems: "flex-start" }}>
          <div
            style={{
              width: 42,
              height: 42,
              borderRadius: 12,
              background: "var(--accent-subtle)",
              color: "var(--accent)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <ResourceIcon size={20} />
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
              <h3 style={{ fontSize: 16, fontWeight: 700 }}>{resourceTitle}</h3>
              {queue.length > 1 && (
                <span className="badge badge-muted">
                  {uiText(`队列 ${queue.length}`, `Queue ${queue.length}`, `キュー ${queue.length}`)}
                </span>
              )}
              {current.enabled && (
                <span className="badge badge-success">
                  {uiText("导入后立即启用", "Enable After Import", "インポート後に有効化")}
                </span>
              )}
            </div>
            <p style={{ marginTop: 6, fontSize: 13, color: "var(--text-muted)", lineHeight: 1.6 }}>
              {uiText(
                "确认后将把该 Deep Link 内容导入到当前 CCHub 环境。仅导入你信任来源的链接。",
                "This deep link will be imported into the current CCHub environment after confirmation. Only import links from trusted sources.",
                "確認後、この Deep Link を現在の CCHub 環境へ取り込みます。信頼できるリンクのみをインポートしてください。",
              )}
            </p>
          </div>
        </div>

        <div style={{ marginTop: 18, display: "grid", gap: 14 }}>
          {current.resource === "provider" && (
            <ProviderPreviewSection
              current={current}
              unnamedLabel={providerUnnamedLabel}
              primaryEndpointLabel={primaryEndpointLabel}
              endpointCandidatesLabel={endpointCandidatesLabel}
              homepageLabel={homepageLabel}
            />
          )}

          {current.resource === "prompt" && (
            <PromptPreviewSection
              current={current}
              unnamedLabel={promptUnnamedLabel}
              contentPreviewLabel={contentPreviewLabel}
              emptyContentLabel={emptyContentLabel}
            />
          )}

          {current.resource === "mcp" && (
            <McpPreviewSection
              current={current}
              unavailablePreviewLabel={unavailablePreviewLabel}
            />
          )}

          {current.resource === "skill" && (
            <SkillPreviewSection
              current={current}
              fetchDescription={skillFetchDescription}
            />
          )}

          <div
            style={{
              display: "flex",
              gap: 10,
              alignItems: "flex-start",
              padding: "12px 14px",
              borderRadius: 10,
              background: "var(--warning-subtle)",
              border: "1px solid var(--warning)",
            }}
          >
            <AlertTriangle size={16} style={{ color: "var(--warning)", flexShrink: 0, marginTop: 1 }} />
            <div style={{ fontSize: 12, lineHeight: 1.6, color: "var(--text-secondary)" }}>
              {uiText(
                "Deep Link 可能包含 API Key、脚本或远程仓库信息。请确认来源可信后再导入。",
                "Deep links may contain API keys, scripts, or remote repositories. Confirm the source is trusted before importing.",
                "Deep Link には API Key、スクリプト、リモートリポジトリ情報が含まれる場合があります。信頼できるソースか確認してからインポートしてください。",
              )}
            </div>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
          <button className="btn btn-secondary btn-sm" onClick={handleCancel} disabled={importing}>
            {uiText("取消", "Cancel", "キャンセル")}
          </button>
          <button className="btn btn-primary btn-sm" onClick={handleConfirm} disabled={importing}>
            {importing
              ? uiText("导入中...", "Importing...", "インポート中...")
              : uiText("确认导入", "Import", "インポート")}
          </button>
        </div>
      </div>
    </div>
  );
}

export default memo(DeepLinkImportDialogComponent);
