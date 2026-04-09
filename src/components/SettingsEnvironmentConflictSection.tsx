import { memo } from "react";
import { AlertCircle, CheckCircle, RefreshCw } from "lucide-react";
import type { Locale } from "../lib/i18n";
import type { EnvironmentConflict } from "../lib/appPreferences";
import SettingsEnvironmentConflictCard from "./SettingsEnvironmentConflictCard";

interface SettingsEnvironmentConflictSectionProps {
  locale: Locale;
  conflicts: EnvironmentConflict[];
  refreshing: boolean;
  checkingLabel: string;
  refreshLabel: string;
  onRefresh: () => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsEnvironmentConflictSectionComponent({
  locale,
  conflicts,
  refreshing,
  checkingLabel,
  refreshLabel,
  onRefresh,
}: SettingsEnvironmentConflictSectionProps) {
  const hasConflicts = conflicts.length > 0;

  return (
    <div className="section-card">
      <div className="section-card-title">
        <AlertCircle size={17} style={{ color: hasConflicts ? "var(--warning)" : "var(--text-secondary)" }} />
        {uiText(locale, "环境变量冲突检测", "Environment Override Detection", "環境変数上書き検出")}
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "flex-start", flexWrap: "wrap", marginBottom: 14 }}>
        <p style={{ fontSize: 12, color: "var(--text-muted)", maxWidth: 720 }}>
          {uiText(
            locale,
            "检测当前桌面进程继承的 CLI 环境变量。若这些变量已存在，可能覆盖 CCHub 的配置文件、Profile 切换和端点设置。",
            "Detect inherited CLI environment variables that may override CCHub-managed config files, profile switching, and endpoint settings.",
            "デスクトッププロセスが引き継いだ CLI 環境変数を検出します。存在する場合、CCHub 管理の設定ファイル、Profile 切替、エンドポイント設定を上書きする可能性があります。",
          )}
        </p>
        <button
          className="btn btn-secondary btn-sm"
          onClick={onRefresh}
          disabled={refreshing}
          style={{ gap: 6 }}
        >
          <RefreshCw size={14} className={refreshing ? "spin" : ""} />
          {refreshing ? checkingLabel : refreshLabel}
        </button>
      </div>
      {hasConflicts ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          {conflicts.map((conflict) => (
            <SettingsEnvironmentConflictCard
              key={conflict.id}
              conflict={conflict}
              locale={locale}
            />
          ))}
        </div>
      ) : (
        <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: "var(--text-secondary)" }}>
          <CheckCircle size={16} style={{ color: "var(--success)" }} />
          {uiText(locale, "未发现会覆盖配置的环境变量", "No overriding environment variables were detected", "設定を上書きする環境変数は検出されませんでした")}
        </div>
      )}
    </div>
  );
}

export default memo(SettingsEnvironmentConflictSectionComponent);
