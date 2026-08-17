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
  maskConfigValue,
  classifyDeepLinkCommand,
  classifyDeepLinkEndpoint,
  classifyDeepLinkEnvKey,
  parseMcpPreviewServers,
  splitDeepLinkEndpoints,
  type DeepLinkErrorPayload,
  type DeepLinkImportRequest,
  type DeepLinkMcpImportResult,
} from "../lib/deeplink";
import {
  useActivatePromptPresetMutation,
  useApplyConfigProfileMutation,
  useSaveConfigProfileMutation,
  useSavePromptPresetMutation,
} from "../hooks/mutations";
import { normalizeDirectory, requestFingerprint, SkillPreviewSection } from "./deeplinkImportHelpers";

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
  usageAccessTokenLabel: string;
  usageUserIdLabel: string;
  usageScriptLabel: string;
  usageScriptCodeLabel: string;
  usageScriptEnabledLabel: string;
  usageScriptDisabledLabel: string;
  usageScriptWarningLabel: string;
  usageApiKeyLabel: string;
  usageBaseUrlLabel: string;
  usageIntervalLabel: string;
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
  labels: {
    command: string;
    args: string;
    url: string;
    env: string;
    headers: string;
    privateEndpointRisk: string;
    envHijackRisk: string;
    shellCommandRisk: string;
    importWarning: string;
  };
}

function ProviderPreviewSectionComponent({
  current,
  unnamedLabel,
  primaryEndpointLabel,
  endpointCandidatesLabel,
  homepageLabel,
  usageAccessTokenLabel,
  usageUserIdLabel,
  usageScriptLabel,
  usageScriptCodeLabel,
  usageScriptEnabledLabel,
  usageScriptDisabledLabel,
  usageScriptWarningLabel,
  usageApiKeyLabel,
  usageBaseUrlLabel,
  usageIntervalLabel,
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
            <div style={{ fontSize: 13, fontFamily: "'JetBrains Mono', monospace" }}>{maskSecret(current.apiKey)}</div>
          </div>
        )}
        {current.usageAccessToken && (
          <div>
            <div className="field-label">{usageAccessTokenLabel}</div>
            <div style={{ fontSize: 13, fontFamily: "'JetBrains Mono', monospace" }}>
              {maskSecret(current.usageAccessToken)}
            </div>
          </div>
        )}
        {current.usageUserId && (
          <div>
            <div className="field-label">{usageUserIdLabel}</div>
            <div style={{ fontSize: 13, wordBreak: "break-all" }}>{current.usageUserId}</div>
          </div>
        )}
        {(current.usageScript ||
          current.usageEnabled !== undefined ||
          current.usageApiKey ||
          current.usageBaseUrl ||
          current.usageAutoInterval !== undefined) && (
          <div style={{ display: "grid", gap: 8, paddingTop: 8, borderTop: "1px solid var(--border-default)" }}>
            <div className="field-label">{usageScriptLabel}</div>
            {(current.usageScript || current.usageEnabled !== undefined) && (
              <span className={`badge ${current.usageEnabled === true ? "badge-success" : "badge-muted"}`}>
                {current.usageEnabled === true ? usageScriptEnabledLabel : usageScriptDisabledLabel}
              </span>
            )}
            {current.usageScript && (
              <>
                <div className="field-label">{usageScriptCodeLabel}</div>
                <pre
                  style={{
                    maxHeight: 220,
                    overflow: "auto",
                    whiteSpace: "pre-wrap",
                    overflowWrap: "anywhere",
                    margin: 0,
                    padding: 10,
                    borderRadius: 8,
                    background: "var(--bg-input)",
                    border: "1px solid var(--border-default)",
                    fontSize: 12,
                    fontFamily: "'JetBrains Mono', monospace",
                  }}
                >
                  {decodeDeepLinkText(current.usageScript)}
                </pre>
                <div
                  style={{ display: "flex", gap: 7, alignItems: "flex-start", color: "var(--warning)", fontSize: 12 }}
                >
                  <AlertTriangle size={14} style={{ flexShrink: 0, marginTop: 1 }} />
                  <span>{usageScriptWarningLabel}</span>
                </div>
              </>
            )}
            {current.usageApiKey && (
              <div style={{ fontSize: 12, fontFamily: "'JetBrains Mono', monospace" }}>
                {usageApiKeyLabel}: {maskSecret(current.usageApiKey)}
              </div>
            )}
            {current.usageBaseUrl && (
              <div style={{ fontSize: 12, wordBreak: "break-all" }}>
                {usageBaseUrlLabel}: {current.usageBaseUrl}
              </div>
            )}
            {current.usageAutoInterval !== undefined && (
              <div style={{ fontSize: 12 }}>
                {usageIntervalLabel}: {current.usageAutoInterval}
              </div>
            )}
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

function McpPreviewSectionComponent({ current, unavailablePreviewLabel, labels }: McpPreviewSectionProps) {
  const mcpServers = useMemo(() => parseMcpPreviewServers(current), [current]);
  const appBadges = useMemo(
    () =>
      (current.apps || "")
        .split(",")
        .map((app) => app.trim())
        .filter(Boolean),
    [current.apps],
  );

  const riskLabels = {
    privateEndpoint: labels.privateEndpointRisk,
    envHijack: labels.envHijackRisk,
    shellCommand: labels.shellCommandRisk,
  } as const;
  const risks = useMemo(() => {
    const found = new Set<keyof typeof riskLabels>();
    for (const server of mcpServers) {
      const commandRisk = classifyDeepLinkCommand(server.command, server.args);
      if (commandRisk) found.add(commandRisk);
      const endpointRisk = server.url ? classifyDeepLinkEndpoint(server.url) : null;
      if (endpointRisk) found.add(endpointRisk);
      for (const key of server.envKeys) {
        const envRisk = classifyDeepLinkEnvKey(key);
        if (envRisk) found.add(envRisk);
      }
    }
    return [...found];
  }, [mcpServers]);

  return (
    <section className="section-card" style={{ padding: 14 }}>
      <div className="field-label">MCP</div>
      <div style={{ display: "grid", gap: 12 }}>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {appBadges.map((app) => (
            <span key={app} className="badge badge-accent">
              {app}
            </span>
          ))}
        </div>
        {mcpServers.length > 0 ? (
          <div style={{ display: "grid", gap: 10 }}>
            {mcpServers.map((server) => (
              <div
                key={server.name}
                style={{
                  padding: "12px 14px",
                  borderRadius: 8,
                  background: "var(--bg-input)",
                  border: "1px solid var(--border-default)",
                }}
              >
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
                  <strong style={{ fontSize: 13 }}>{server.name}</strong>
                  <span className="badge badge-muted">{server.transport}</span>
                </div>
                {server.command && (
                  <div
                    style={{
                      marginTop: 8,
                      display: "grid",
                      gap: 4,
                      fontSize: 12,
                      fontFamily: "'JetBrains Mono', monospace",
                      wordBreak: "break-all",
                    }}
                  >
                    <div>
                      {labels.command}: {server.command}
                    </div>
                    {server.args.map((arg, index) => (
                      <div key={`${server.name}-arg-${index}`}>
                        {index === 0 ? `${labels.args}: ` : ""}
                        {arg}
                      </div>
                    ))}
                  </div>
                )}
                {server.url && (
                  <div
                    style={{
                      marginTop: 8,
                      fontSize: 12,
                      fontFamily: "'JetBrains Mono', monospace",
                      wordBreak: "break-all",
                      color: classifyDeepLinkEndpoint(server.url) ? "var(--warning)" : "var(--text-secondary)",
                    }}
                  >
                    {labels.url}: {server.url}
                  </div>
                )}
                {server.envKeys.length > 0 && (
                  <div
                    style={{
                      marginTop: 8,
                      display: "grid",
                      gap: 4,
                      fontSize: 12,
                      fontFamily: "'JetBrains Mono', monospace",
                      wordBreak: "break-all",
                    }}
                  >
                    {Object.entries({ ...server.env, ...server.headers }).map(([key, value], index) => (
                      <div
                        key={`${server.name}-${key}`}
                        style={{ color: classifyDeepLinkEnvKey(key) ? "var(--warning)" : "var(--text-secondary)" }}
                      >
                        {index === 0 ? `${server.headers[key] !== undefined ? labels.headers : labels.env}: ` : ""}
                        {key}={maskConfigValue(key, value)}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        ) : (
          <div style={{ fontSize: 13, color: "var(--text-secondary)" }}>{unavailablePreviewLabel}</div>
        )}
        {risks.length > 0 && (
          <div
            style={{
              display: "grid",
              gap: 5,
              padding: "10px 12px",
              borderRadius: 8,
              background: "var(--warning-subtle)",
              border: "1px solid var(--warning)",
              color: "var(--warning)",
              fontSize: 12,
            }}
          >
            {risks.map((risk) => (
              <div key={risk} style={{ display: "flex", gap: 7, alignItems: "flex-start" }}>
                <AlertTriangle size={14} style={{ flexShrink: 0, marginTop: 1 }} />
                <span>{riskLabels[risk]}</span>
              </div>
            ))}
          </div>
        )}
        <div style={{ display: "flex", gap: 7, alignItems: "flex-start", color: "var(--warning)", fontSize: 12 }}>
          <AlertTriangle size={14} style={{ flexShrink: 0, marginTop: 1 }} />
          <span>{labels.importWarning}</span>
        </div>
      </div>
    </section>
  );
}

const McpPreviewSection = memo(McpPreviewSectionComponent);

function DeepLinkImportDialogComponent() {
  const [queue, setQueue] = useState<DeepLinkImportRequest[]>([]);
  const [importing, setImporting] = useState(false);
  const locale = getLocale();
  const saveConfigProfileMutation = useSaveConfigProfileMutation<string>();
  const applyConfigProfileMutation = useApplyConfigProfileMutation();
  const savePromptPresetMutation = useSavePromptPresetMutation<PromptPreset>();
  const activatePromptPresetMutation = useActivatePromptPresetMutation();
  const current = queue[0] || null;

  const uiText = useCallback(
    (zhText: string, enText: string, jaText?: string) =>
      locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText,
    [locale],
  );

  const enqueueRequest = useCallback(
    async (request: DeepLinkImportRequest, disposed = false) => {
      try {
        const prepared =
          request.resource === "provider" && (request.config || request.configUrl)
            ? await deeplinkApi.mergeRequest(request)
            : request;
        if (disposed) return;
        setQueue((currentQueue) =>
          currentQueue.some((item) => requestFingerprint(item) === requestFingerprint(prepared))
            ? currentQueue
            : [...currentQueue, prepared],
        );
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
    },
    [uiText],
  );

  const loadPending = useCallback(
    async (disposed = false) => {
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
    },
    [enqueueRequest, uiText],
  );

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

  const resolveSkill = useCallback(
    async (request: DeepLinkImportRequest): Promise<SkillRegistryEntry> => {
      const repo = request.repo?.trim();
      if (!repo) {
        throw new Error(
          uiText(
            "技能 Deep Link 缺少 repo 参数",
            "Skill deep link is missing repo",
            "Skill Deep Link に repo がありません",
          ),
        );
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

      throw new Error(
        uiText(
          "技能仓库包含多个技能，请在 Deep Link 中补充 directory 参数",
          "Skill repository contains multiple skills. Add a directory parameter to the deep link.",
          "Skill リポジトリに複数のスキルがあります。Deep Link に directory を指定してください。",
        ),
      );
    },
    [uiText],
  );

  const handleConfirm = useCallback(async () => {
    if (!current || importing) return;
    setImporting(true);
    try {
      if (current.resource === "provider") {
        const profile = buildProviderProfileFromDeepLink(current);
        const profileId = await saveConfigProfileMutation.mutateAsync({
          name: profile.name,
          toolId: profile.toolId,
          configSnapshot: profile.configSnapshot,
        });
        if (current.enabled) {
          await applyConfigProfileMutation.mutateAsync(profileId);
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
        const preset = await savePromptPresetMutation.mutateAsync({
          id: null,
          name: current.name?.trim() || uiText("导入提示词", "Imported Prompt", "インポートした Prompt"),
          content: decodeDeepLinkText(current.content),
        });
        if (current.enabled) {
          await activatePromptPresetMutation.mutateAsync({ id: preset.id });
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
        throw new Error(
          uiText("暂不支持的 Deep Link 类型", "Unsupported deep link type", "未対応の Deep Link 種別です"),
        );
      }

      setQueue((currentQueue) => currentQueue.slice(1));
    } catch (error) {
      showToast("error", uiText(`导入失败: ${error}`, `Import failed: ${error}`, `インポートに失敗しました: ${error}`));
    } finally {
      setImporting(false);
    }
  }, [
    activatePromptPresetMutation,
    applyConfigProfileMutation,
    current,
    importing,
    resolveSkill,
    saveConfigProfileMutation,
    savePromptPresetMutation,
    uiText,
  ]);

  const handleCancel = useCallback(() => {
    if (importing) return;
    setQueue((currentQueue) => currentQueue.slice(1));
  }, [importing]);

  const resourceTitle = current
    ? current.resource === "provider"
      ? uiText("导入 Provider", "Import Provider", "Provider をインポート")
      : current.resource === "prompt"
        ? uiText("导入提示词", "Import Prompt", "Prompt をインポート")
        : current.resource === "mcp"
          ? uiText("导入 MCP", "Import MCP", "MCP をインポート")
          : uiText("导入技能", "Import Skill", "Skill をインポート")
    : "";
  const ResourceIcon = current
    ? current.resource === "provider"
      ? Globe
      : current.resource === "prompt"
        ? FileText
        : current.resource === "mcp"
          ? Wrench
          : Package
    : Package;
  const providerUnnamedLabel = uiText("未命名 Provider", "Unnamed Provider", "無名の Provider");
  const promptUnnamedLabel = uiText("未命名提示词", "Unnamed Prompt", "無名の Prompt");
  const primaryEndpointLabel = uiText("主端点", "Primary Endpoint", "プライマリエンドポイント");
  const endpointCandidatesLabel = uiText("候选端点", "Endpoint Candidates", "候補エンドポイント");
  const homepageLabel = uiText("主页", "Homepage", "ホームページ");
  const usageAccessTokenLabel = uiText("用量访问令牌", "Usage Access Token", "使用量アクセストークン");
  const usageUserIdLabel = uiText("用量用户 ID", "Usage User ID", "使用量ユーザー ID");
  const usageScriptLabel = uiText("用量查询脚本", "Usage Query Script", "使用量クエリスクリプト");
  const usageScriptCodeLabel = uiText("脚本正文", "Script Code", "スクリプトコード");
  const usageScriptEnabledLabel = uiText("已启用", "Enabled", "有効");
  const usageScriptDisabledLabel = uiText("未启用（默认）", "Disabled by default", "デフォルト無効");
  const usageScriptWarningLabel = uiText(
    "启用后会在查询用量时执行此 JavaScript，请先确认来源可信。",
    "This JavaScript runs during usage queries when enabled. Verify the source before importing.",
    "有効にすると使用量クエリでこの JavaScript が実行されます。インポート前にソースを確認してください。",
  );
  const usageApiKeyLabel = uiText("用量 API Key", "Usage API Key", "使用量 API キー");
  const usageBaseUrlLabel = uiText("用量地址", "Usage Base URL", "使用量ベース URL");
  const usageIntervalLabel = uiText("自动查询间隔（分钟）", "Auto query interval (minutes)", "自動クエリ間隔（分）");
  const contentPreviewLabel = uiText("内容预览", "Content Preview", "内容プレビュー");
  const emptyContentLabel = uiText("无内容", "No content", "内容なし");
  const unavailablePreviewLabel = uiText(
    "无法预览 MCP 配置，将在导入时校验。",
    "MCP config preview is unavailable and will be validated on import.",
    "MCP 設定はプレビューできません。インポート時に検証します。",
  );
  const mcpPreviewLabels = {
    command: uiText("命令", "Command", "コマンド"),
    args: uiText("参数", "Args", "引数"),
    url: uiText("远程 URL", "Remote URL", "リモート URL"),
    env: uiText("环境变量", "Environment", "環境変数"),
    headers: uiText("请求头", "Headers", "ヘッダー"),
    privateEndpointRisk: uiText(
      "该地址指向本机或私有网络，导入前请确认。",
      "This endpoint points to a local or private network. Verify it before importing.",
      "このエンドポイントはローカルまたはプライベートネットワークを指します。確認してください。",
    ),
    envHijackRisk: uiText(
      "环境变量可能改变进程加载或网络代理行为。",
      "This environment variable can alter process loading or network proxy behavior.",
      "この環境変数はプロセスの読み込みまたはネットワークプロキシを変更できます。",
    ),
    shellCommandRisk: uiText(
      "该命令会通过 Shell 解释器执行内联命令。",
      "This command executes an inline command through a shell interpreter.",
      "このコマンドはシェルインタープリター経由でインラインコマンドを実行します。",
    ),
    importWarning: uiText(
      "MCP 会在目标工具启动时执行命令或连接远程服务。",
      "MCP commands may execute or connect to remote services when the target tool starts.",
      "MCP は対象ツールの起動時にコマンドを実行するか、リモートサービスへ接続します。",
    ),
  };
  const skillFetchDescription = uiText(
    "确认时将从远程仓库拉取技能内容并安装到当前技能目录。",
    "The skill content will be fetched from the repository and installed into the current skills directory on confirmation.",
    "確認時にリポジトリから Skill を取得し、現在の Skill ディレクトリへインストールします。",
  );

  if (!current) return null;

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
              usageAccessTokenLabel={usageAccessTokenLabel}
              usageUserIdLabel={usageUserIdLabel}
              usageScriptLabel={usageScriptLabel}
              usageScriptCodeLabel={usageScriptCodeLabel}
              usageScriptEnabledLabel={usageScriptEnabledLabel}
              usageScriptDisabledLabel={usageScriptDisabledLabel}
              usageScriptWarningLabel={usageScriptWarningLabel}
              usageApiKeyLabel={usageApiKeyLabel}
              usageBaseUrlLabel={usageBaseUrlLabel}
              usageIntervalLabel={usageIntervalLabel}
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
              labels={mcpPreviewLabels}
            />
          )}

          {current.resource === "skill" && (
            <SkillPreviewSection current={current} fetchDescription={skillFetchDescription} />
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
