import { memo } from "react";
import { Info } from "lucide-react";
import type { Locale } from "../lib/i18n";
import type { LogFileTargets } from "../lib/appPreferences";
import SettingsLogFileCard from "./SettingsLogFileCard";
import SettingsLogLevelCard from "./SettingsLogLevelCard";

interface SettingsLoggingSectionProps {
  locale: Locale;
  level: string;
  saving: boolean;
  logFileTargets: LogFileTargets | null;
  onSaveLogLevel: (level: string) => void | Promise<void>;
  onCopy: (value: string, label: string) => void | Promise<void>;
  onOpen: (target: string, label: string) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsLoggingSectionComponent({
  locale,
  level,
  saving,
  logFileTargets,
  onSaveLogLevel,
  onCopy,
  onOpen,
}: SettingsLoggingSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Info size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "日志级别", "Log Level", "ログレベル")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
        {uiText(
          locale,
          "控制运行期日志详细程度。设置会立即影响 `~/.cchub/app.log` 的写入阈值，`crash.log` 仍持续记录崩溃信息。",
          "Control runtime log verbosity. Changes take effect immediately for `~/.cchub/app.log`, while `crash.log` continues to capture crash reports.",
          "実行時ログの詳細度を制御します。設定は `~/.cchub/app.log` の出力しきい値へ即時反映され、`crash.log` は引き続きクラッシュ情報を記録します。",
        )}
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))", gap: 10, marginBottom: 14 }}>
        {[
          ["error", uiText(locale, "仅记录错误与崩溃", "Errors and crashes only", "エラーとクラッシュのみ")],
          ["warn", uiText(locale, "记录警告、错误与崩溃", "Warnings, errors, and crashes", "警告・エラー・クラッシュ")],
          ["info", uiText(locale, "记录常规操作与状态", "Operational events and status", "通常操作と状態")],
          ["debug", uiText(locale, "记录详细调试步骤", "Detailed debugging steps", "詳細なデバッグ手順")],
          ["trace", uiText(locale, "记录最细粒度诊断信息", "Most verbose diagnostics", "最も詳細な診断情報")],
        ].map(([itemLevel, description]) => (
          <SettingsLogLevelCard
            key={itemLevel}
            level={itemLevel}
            description={description}
            active={level === itemLevel}
            disabled={saving}
            onSelect={onSaveLogLevel}
          />
        ))}
      </div>
      {logFileTargets && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))", gap: 10 }}>
          {[
            ["app.log", logFileTargets.runtime_log_path, uiText(locale, "运行期操作日志", "Runtime activity log", "実行時アクティビティログ")],
            ["crash.log", logFileTargets.crash_log_path, uiText(locale, "崩溃与 panic 记录", "Crash and panic log", "クラッシュと panic のログ")],
          ].map(([label, path, description]) => (
            <SettingsLogFileCard
              key={label}
              label={label}
              path={path}
              description={description}
              locale={locale}
              onCopy={onCopy}
              onOpen={onOpen}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default memo(SettingsLoggingSectionComponent);
