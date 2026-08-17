import { memo } from "react";
import { Link2 } from "lucide-react";
import CopilotAuthSection from "./CopilotAuthSection";
import LocalAuthStatusPanel from "./LocalAuthStatusPanel";
import CopilotUsagePanel from "./CopilotUsagePanel";
import CodexUsagePanel from "./CodexUsagePanel";
import CodexOAuthAuthSection from "./CodexOAuthAuthSection";
import XaiOAuthAuthSection from "./XaiOAuthAuthSection";
import ProviderHealthPanel from "./ProviderHealthPanel";
import type { Locale } from "../lib/i18n";

interface SettingsAuthCenterSectionProps {
  locale: Locale;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsAuthCenterSectionComponent({ locale }: SettingsAuthCenterSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "Auth Center", "Auth Center", "Auth Center")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 16 }}>
        {uiText(
          locale,
          "集中查看本机 OAuth 凭据、Copilot 账号以及 Claude/Codex 的官方订阅用量。查询只读，不会在应用内删除 CLI 凭据。",
          "Review local OAuth credentials, Copilot accounts, and official Claude/Codex subscription usage in one place. Queries are read-only and never delete CLI credentials.",
          "ローカル OAuth 資格情報、Copilot アカウント、Claude/Codex の公式利用量をまとめて確認できます。照会は読み取り専用で、CLI の資格情報を削除しません。",
        )}
      </p>
      <CopilotAuthSection />
      <CodexOAuthAuthSection localeText={(zh, en, ja) => uiText(locale, zh, en, ja)} />
      <XaiOAuthAuthSection localeText={(zh, en, ja) => uiText(locale, zh, en, ja)} />
      <CopilotUsagePanel localeText={(zh, en, ja) => uiText(locale, zh, en, ja)} />
      <CodexUsagePanel localeText={(zh, en, ja) => uiText(locale, zh, en, ja)} />
      <LocalAuthStatusPanel localeText={(zh, en, ja) => uiText(locale, zh, en, ja)} />
      <ProviderHealthPanel localeText={(zh, en, ja) => uiText(locale, zh, en, ja)} />
    </div>
  );
}

export default memo(SettingsAuthCenterSectionComponent);
