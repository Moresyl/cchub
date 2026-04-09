import { memo } from "react";
import { Copy } from "lucide-react";
import { getAppLabel, type ManagedAppId } from "../lib/appPreferences";
import type { Locale } from "../lib/i18n";

interface SettingsProxyEndpointCardProps {
  appId: ManagedAppId;
  endpoint: string;
  locale: Locale;
  onCopy: (value: string, label: string) => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsProxyEndpointCardComponent({
  appId,
  endpoint,
  locale,
  onCopy,
}: SettingsProxyEndpointCardProps) {
  const appLabel = getAppLabel(appId);
  return (
    <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center", marginBottom: 8 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{appLabel}</div>
        <button
          className="btn btn-ghost btn-icon-sm"
          onClick={() => void onCopy(endpoint, `${appLabel} Proxy URL`)}
          title={uiText(locale, "复制代理地址", "Copy proxy URL", "プロキシ URL をコピー")}
        >
          <Copy size={12} />
        </button>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "'JetBrains Mono', monospace", wordBreak: "break-all" }}>
        {endpoint}
      </div>
    </div>
  );
}

export default memo(SettingsProxyEndpointCardComponent);
