import { memo } from "react";
import { AlertCircle, CheckCircle, Download, RefreshCw } from "lucide-react";

interface SettingsAppUpdateSectionLabels {
  appUpdate: string;
  currentVersion: string;
  checkForUpdate: string;
  checking: string;
  updateAvailable: string;
  downloading: string;
  installUpdate: string;
  updateNotConfigured: string;
  noUpdate: string;
  updateFailed: string;
  disabledTitle: string;
  disabledDescription: string;
  disabledCheckDescription: string;
  openDownloadsLabel: string;
  releaseOnlyDescription: string;
}

interface SettingsAppUpdateSectionAppUpdate {
  update_available: boolean;
  latest_version: string | null;
  current_version: string | null;
  body: string | null;
  not_configured: boolean;
  can_install: boolean;
  release_url: string | null;
  source: string | null;
  disabled_by_env?: boolean;
}

interface SettingsAppUpdateSectionProps {
  labels: SettingsAppUpdateSectionLabels;
  appVersion: string;
  checkingUpdate: boolean;
  installing: boolean;
  updaterDisabledByEnv: boolean;
  updaterEnvValue: string | null;
  appUpdate: SettingsAppUpdateSectionAppUpdate | null;
  updateError: string | null;
  onCheckUpdate: () => void | Promise<void>;
  onInstallUpdate: () => void | Promise<void>;
}

function SettingsAppUpdateSectionComponent({
  labels,
  appVersion,
  checkingUpdate,
  installing,
  updaterDisabledByEnv,
  updaterEnvValue,
  appUpdate,
  updateError,
  onCheckUpdate,
  onInstallUpdate,
}: SettingsAppUpdateSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Download size={17} style={{ color: "var(--text-secondary)" }} />
        {labels.appUpdate}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <p style={{ fontSize: 14, fontWeight: 500 }}>{labels.currentVersion}</p>
            <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 2 }}>{appVersion}</p>
          </div>
          <button
            className="btn btn-sm btn-secondary"
            onClick={onCheckUpdate}
            disabled={checkingUpdate || updaterDisabledByEnv}
            style={{ gap: 6 }}
          >
            <RefreshCw size={14} className={checkingUpdate ? "spin" : ""} />
            {checkingUpdate ? labels.checking : labels.checkForUpdate}
          </button>
        </div>

        {updaterDisabledByEnv && (
          <>
            <div className="divider" />
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 8,
                padding: "12px 14px",
                borderRadius: 10,
                background: "var(--bg-input)",
                border: "1px solid color-mix(in srgb, var(--warning) 55%, transparent)",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                <span style={{ fontSize: 13, fontWeight: 600, color: "var(--warning)" }}>
                  {labels.disabledTitle}
                </span>
              </div>
              <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>
                {labels.disabledDescription}
              </p>
              {updaterEnvValue && (
                <code
                  className="badge badge-accent"
                  style={{ alignSelf: "flex-start", fontSize: 11, fontFamily: "'JetBrains Mono', monospace" }}
                >
                  {`DISABLE_AUTOUPDATER=${updaterEnvValue}`}
                </code>
              )}
            </div>
          </>
        )}

        {appUpdate && (
          <>
            <div className="divider" />
            {appUpdate.disabled_by_env ? (
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                <span style={{ fontSize: 13, color: "var(--warning)" }}>
                  {labels.disabledCheckDescription}
                </span>
              </div>
            ) : appUpdate.update_available ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <AlertCircle size={16} style={{ color: "var(--warning)" }} />
                  <span style={{ fontSize: 14, fontWeight: 500, color: "var(--warning)" }}>
                    {labels.updateAvailable}: v{appUpdate.latest_version}
                  </span>
                </div>
                {appUpdate.body && (
                  <p style={{ fontSize: 12, color: "var(--text-secondary)", whiteSpace: "pre-wrap" }}>{appUpdate.body}</p>
                )}
                {!appUpdate.can_install && (
                  <p style={{ fontSize: 12, color: "var(--text-muted)" }}>
                    {labels.releaseOnlyDescription}
                  </p>
                )}
                <button
                  className="btn btn-sm btn-primary"
                  onClick={onInstallUpdate}
                  disabled={installing || updaterDisabledByEnv}
                  style={{ alignSelf: "flex-start", gap: 6 }}
                >
                  <Download size={14} />
                  {installing
                    ? labels.downloading
                    : appUpdate.can_install
                      ? labels.installUpdate
                      : labels.openDownloadsLabel}
                </button>
              </div>
            ) : (
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <CheckCircle size={16} style={{ color: "var(--success)" }} />
                <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>
                  {appUpdate.not_configured
                    ? labels.updateNotConfigured
                    : labels.noUpdate}
                </span>
              </div>
            )}
          </>
        )}

        {updateError && (
          <>
            <div className="divider" />
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <AlertCircle size={16} style={{ color: "var(--error)" }} />
              <span style={{ fontSize: 13, color: "var(--error)" }}>{labels.updateFailed}: {updateError}</span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default memo(SettingsAppUpdateSectionComponent);
