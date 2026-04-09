import { memo } from "react";
import { getAppLabel, isManagedAppId, type EnvironmentConflict } from "../lib/appPreferences";
import type { Locale } from "../lib/i18n";

interface SettingsEnvironmentConflictCardProps {
  conflict: EnvironmentConflict;
  locale: Locale;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function getAffectedAppLabel(appId: string) {
  return isManagedAppId(appId) ? getAppLabel(appId) : appId;
}

function SettingsEnvironmentConflictCardComponent({
  conflict,
  locale,
}: SettingsEnvironmentConflictCardProps) {
  const primaryAppLabel = conflict.affected_apps[0] ? getAffectedAppLabel(conflict.affected_apps[0]) : "Unknown";
  const title = conflict.kind === "multi_tool_override"
    ? uiText(locale, "检测到多套 CLI 环境覆盖", "Multiple CLI override groups detected", "複数の CLI 上書きグループを検出しました")
    : uiText(
      locale,
      `${primaryAppLabel} 环境覆盖`,
      `${primaryAppLabel} override detected`,
      `${primaryAppLabel} の上書きを検出しました`,
    );
  const description = conflict.kind === "multi_tool_override"
    ? uiText(
      locale,
      "多个工具的认证或端点变量同时存在，常见于历史 shell 配置残留。",
      "Auth or endpoint variables for multiple tools are present at the same time, often from old shell profile exports.",
      "複数ツールの認証またはエンドポイント変数が同時に存在します。古い shell 設定の残骸であることが多いです。",
    )
    : uiText(
      locale,
      "这些变量会优先于 CCHub 写入的配置文件生效。",
      "These variables take precedence over configuration files managed by CCHub.",
      "これらの変数は CCHub が管理する設定ファイルより優先されます。",
    );

  return (
    <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap", marginBottom: 8 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{title}</div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {conflict.affected_apps.map((appId) => (
            <span key={appId} className="badge badge-warning" style={{ fontSize: 10 }}>
              {getAffectedAppLabel(appId)}
            </span>
          ))}
        </div>
      </div>
      <div style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 8 }}>
        {description}
      </div>
      <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {conflict.variables.map((item) => (
          <code key={item} className="badge badge-accent" style={{ fontSize: 11, fontFamily: "'JetBrains Mono', monospace" }}>{item}</code>
        ))}
      </div>
    </div>
  );
}

export default memo(SettingsEnvironmentConflictCardComponent);
