import { memo } from "react";
import { FolderOpen, Link2 } from "lucide-react";
import type { Locale } from "../lib/i18n";
import type { TerminalPreferences } from "../lib/appPreferences";
import SettingsTerminalOptionCard from "./SettingsTerminalOptionCard";
import LoadingState from "./states/LoadingState";

interface SettingsPreferredTerminalSectionProps {
  locale: Locale;
  terminalPreferences: TerminalPreferences | null;
  savingTerminal: boolean;
  onSelectTerminal: (terminalId: string) => void | Promise<void>;
  onOpenHomeInTerminal: () => void | Promise<void>;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsPreferredTerminalSectionComponent({
  locale,
  terminalPreferences,
  savingTerminal,
  onSelectTerminal,
  onOpenHomeInTerminal,
}: SettingsPreferredTerminalSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Link2 size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "首选终端", "Preferred Terminal", "優先ターミナル")}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
        {uiText(
          locale,
          "为后续会话恢复和目录打开动作指定默认终端，现在也可以直接用下方按钮验证。",
          "Choose the terminal CCHub should prefer for future session restore and open-in-terminal actions. You can test it below now.",
          "将来のセッション復元やディレクトリをターミナルで開く操作に使う既定ターミナルを選択します。下のボタンですぐ確認できます。",
        )}
      </p>
      {terminalPreferences ? (
        <>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))", gap: 10 }}>
            {terminalPreferences.options.map((option) => (
              <SettingsTerminalOptionCard
                key={option.id}
                option={option}
                active={option.id === terminalPreferences.selected_terminal}
                disabled={savingTerminal}
                locale={locale}
                onSelect={onSelectTerminal}
              />
            ))}
          </div>
          <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap", marginTop: 14 }}>
            <span className="badge badge-accent">
              {uiText(
                locale,
                `当前平台: ${terminalPreferences.platform}`,
                `Platform: ${terminalPreferences.platform}`,
                `現在のプラットフォーム: ${terminalPreferences.platform}`,
              )}
            </span>
            <button className="btn btn-secondary btn-sm" style={{ gap: 6 }} onClick={onOpenHomeInTerminal}>
              <FolderOpen size={14} />
              {uiText(locale, "在终端中打开主目录", "Open Home In Terminal", "ホームをターミナルで開く")}
            </button>
          </div>
        </>
      ) : (
        <LoadingState
          label={uiText(locale, "正在读取终端列表...", "Loading terminal options...", "ターミナル一覧を読み込み中...")}
        />
      )}
    </div>
  );
}

export default memo(SettingsPreferredTerminalSectionComponent);
