import { memo } from "react";
import { Copy, FolderOpen } from "lucide-react";
import type { Locale } from "../lib/i18n";

interface SettingsLogFileCardProps {
  label: string;
  path: string;
  description: string;
  locale: Locale;
  onCopy: (value: string, label: string) => void | Promise<void>;
  onOpen: (target: string, label: string) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsLogFileCardComponent({
  label,
  path,
  description,
  locale,
  onCopy,
  onOpen,
}: SettingsLogFileCardProps) {
  return (
    <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
        <div>
          <div style={{ fontSize: 13, fontWeight: 600 }}>{label}</div>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{description}</div>
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => void onCopy(path, label)}
            title={uiText(locale, "复制路径", "Copy path", "パスをコピー")}
          >
            <Copy size={12} />
          </button>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => void onOpen(path, label)}
            title={uiText(locale, "打开日志", "Open log", "ログを開く")}
          >
            <FolderOpen size={12} />
          </button>
        </div>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
        {path}
      </div>
    </div>
  );
}

export default memo(SettingsLogFileCardComponent);
