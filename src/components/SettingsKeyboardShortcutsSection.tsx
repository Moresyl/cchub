import { memo } from "react";
import { Info } from "lucide-react";
import type { Locale } from "../lib/i18n";
import SettingsShortcutCard from "./SettingsShortcutCard";

interface SettingsKeyboardShortcutsSectionProps {
  locale: Locale;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function SettingsKeyboardShortcutsSectionComponent({ locale }: SettingsKeyboardShortcutsSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Info size={17} style={{ color: "var(--text-secondary)" }} />
        {uiText(locale, "键盘快捷键", "Keyboard Shortcuts", "キーボードショートカット")}
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 10 }}>
        {[
          {
            key: "Ctrl/Cmd + ,",
            desc: uiText(locale, "全局打开设置页", "Open Settings globally", "設定ページを開く"),
          },
          {
            key: "Esc",
            desc: uiText(locale, "关闭技能编辑器、工作流编辑器、指令文档编辑器等面板", "Close editors and drilldown panels such as Skills, Workflows, and Instruction Docs", "スキル、ワークフロー、指示ドキュメントなどの編集・詳細パネルを閉じる"),
          },
          {
            key: "Ctrl/Cmd + S",
            desc: uiText(locale, "在支持的编辑页快速保存", "Save on supported editor pages", "対応する編集ページですばやく保存"),
          },
          {
            key: "Ctrl/Cmd + N",
            desc: uiText(locale, "在支持的页面新建配置、预设或向导", "Create a new profile, preset, or wizard flow on supported pages", "対応ページで新規プロファイル、プリセット、ウィザードを開始"),
          },
          {
            key: "Ctrl/Cmd + F",
            desc: uiText(locale, "聚焦当前页面的主搜索框（Profiles / Skills / Marketplace）", "Focus the primary search field on the current page (Profiles / Skills / Marketplace)", "現在のページの主検索欄へフォーカス（Profiles / Skills / Marketplace）"),
          },
        ].map((item) => (
          <SettingsShortcutCard
            key={item.key}
            shortcutKey={item.key}
            description={item.desc}
          />
        ))}
      </div>
    </div>
  );
}

export default memo(SettingsKeyboardShortcutsSectionComponent);
