import { memo, useMemo } from "react";
import { AlertTriangle } from "lucide-react";
import {
  classifyDeepLinkCommand,
  classifyDeepLinkEndpoint,
  classifyDeepLinkEnvKey,
  decodeDeepLinkText,
  getPrimaryDeepLinkEndpoint,
  maskConfigValue,
  maskSecret,
  parseMcpPreviewServers,
  splitDeepLinkEndpoints,
  type DeepLinkImportRequest,
} from "../../lib/deeplink";

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

export const ProviderPreviewSection = memo(ProviderPreviewSectionComponent);

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

export const PromptPreviewSection = memo(PromptPreviewSectionComponent);

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

export const McpPreviewSection = memo(McpPreviewSectionComponent);
