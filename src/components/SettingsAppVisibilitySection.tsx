import { memo } from "react";
import { Globe } from "lucide-react";
import { MANAGED_APPS, type ManagedAppId } from "../lib/appPreferences";
import type { Locale } from "../lib/i18n";
import SettingsManagedAppToggle from "./SettingsManagedAppToggle";

interface SettingsAppVisibilitySectionProps {
  locale: Locale;
  visibleApps: ManagedAppId[];
  savingVisibleApps: boolean;
  onToggleVisibleApp: (appId: ManagedAppId) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsAppVisibilitySectionComponent({
  locale,
  visibleApps,
  savingVisibleApps,
  onToggleVisibleApp,
}: SettingsAppVisibilitySectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Globe size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "App 可见性", "App Visibility", "App 表示設定")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
        {uiText(
          locale,
          "控制工具页、配置文件、工作流、指令文档等页面中的 App 标签显示。至少保留一个。",
          "Control which app tabs appear across Tools, Config Files, Workflows, Instruction Docs, and related pages. Keep at least one visible.",
          "Tools、設定ファイル、ワークフロー、指示ドキュメントなどで表示する App タブを制御します。少なくとも 1 つは残してください。",
        )}
      </p>
      <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
        {MANAGED_APPS.map((appId) => (
          <SettingsManagedAppToggle
            key={appId}
            appId={appId}
            active={visibleApps.includes(appId)}
            disabled={savingVisibleApps}
            onToggle={onToggleVisibleApp}
          />
        ))}
      </div>
      <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 10 }}>
        {uiText(
          locale,
          `当前显示 ${visibleApps.length} / ${MANAGED_APPS.length} 个 App`,
          `${visibleApps.length} / ${MANAGED_APPS.length} apps currently visible`,
          `${visibleApps.length} / ${MANAGED_APPS.length} 個の App を表示中`,
        )}
      </p>
    </div>
  );
}

export default memo(SettingsAppVisibilitySectionComponent);
