import { memo } from "react";
import { Link2 } from "lucide-react";
import CopilotAuthSection from "./CopilotAuthSection";
import type { Locale } from "../lib/i18n";

interface SettingsAuthCenterSectionProps {
  locale: Locale;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsAuthCenterSectionComponent({
  locale,
}: SettingsAuthCenterSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "Auth Center", "Auth Center", "Auth Center")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
        {uiText(
          locale,
          "集中管理需要 OAuth 的第三方账号。当前已接入 GitHub Copilot，多账号登录后可在 Profiles 页把某个 Provider 绑定到指定账号。",
          "Manage OAuth-backed third-party accounts in one place. GitHub Copilot is wired in now; after adding accounts you can bind a provider to a specific account from Profiles.",
          "OAuth が必要なサードパーティアカウントをここでまとめて管理します。現在は GitHub Copilot に対応しており、追加後は Profiles で Provider ごとにアカウントを紐付けできます。",
        )}
      </p>
      <CopilotAuthSection />
    </div>
  );
}

export default memo(SettingsAuthCenterSectionComponent);
