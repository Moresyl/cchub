import { memo } from "react";
import type { Locale } from "../lib/i18n";
import type { TerminalOption } from "../lib/appPreferences";

interface SettingsTerminalOptionCardProps {
  option: TerminalOption;
  active: boolean;
  disabled: boolean;
  locale: Locale;
  onSelect: (terminalId: string) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsTerminalOptionCardComponent({
  option,
  active,
  disabled,
  locale,
  onSelect,
}: SettingsTerminalOptionCardProps) {
  return (
    <button
      type="button"
      onClick={() => onSelect(option.id)}
      disabled={disabled}
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        border: `1px solid ${active ? "var(--text-primary)" : "var(--border-default)"}`,
        background: active ? "var(--bg-elevated)" : "var(--bg-input)",
        textAlign: "left",
        cursor: "pointer",
        opacity: option.installed ? 1 : 0.55,
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
        <span style={{ fontSize: 13, fontWeight: 600 }}>{option.label}</span>
        <span className={`badge ${option.installed ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
          {option.installed ? uiText(locale, "已检测", "Detected", "検出済み") : uiText(locale, "未检测", "Missing", "未検出")}
        </span>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>{option.command}</div>
    </button>
  );
}

export default memo(SettingsTerminalOptionCardComponent);
