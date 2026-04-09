import { memo } from "react";
import { Globe, Moon, Palette, Sun } from "lucide-react";
import type { Locale } from "../lib/i18n";
import type { Theme } from "../lib/theme";
import SettingsChoiceButton from "./SettingsChoiceButton";

interface SettingsAppearanceSectionProps {
  title: string;
  themeLabel: string;
  languageLabel: string;
  darkLabel: string;
  lightLabel: string;
  theme: Theme;
  locale: Locale;
  onThemeChange: (theme: Theme) => void | Promise<void>;
  onLocaleChange: (locale: Locale) => void | Promise<void>;
}

function SettingsAppearanceSectionComponent({
  title,
  themeLabel,
  languageLabel,
  darkLabel,
  lightLabel,
  theme,
  locale,
  onThemeChange,
  onLocaleChange,
}: SettingsAppearanceSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Palette size={17} style={{ color: "var(--text-secondary)" }} />
        {title}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <p style={{ fontSize: 14, fontWeight: 500 }}>{themeLabel}</p>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <SettingsChoiceButton
              value="dark"
              label={darkLabel}
              active={theme === "dark"}
              icon={Moon}
              onSelect={onThemeChange}
            />
            <SettingsChoiceButton
              value="light"
              label={lightLabel}
              active={theme === "light"}
              icon={Sun}
              onSelect={onThemeChange}
            />
          </div>
        </div>

        <div className="divider" />

        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <Globe size={16} style={{ color: "var(--text-secondary)" }} />
            <p style={{ fontSize: 14, fontWeight: 500 }}>{languageLabel}</p>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <SettingsChoiceButton
              value="zh"
              label="中文"
              active={locale === "zh"}
              onSelect={onLocaleChange}
            />
            <SettingsChoiceButton
              value="en"
              label="English"
              active={locale === "en"}
              onSelect={onLocaleChange}
            />
            <SettingsChoiceButton
              value="ja"
              label="日本語"
              active={locale === "ja"}
              onSelect={onLocaleChange}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

export default memo(SettingsAppearanceSectionComponent);
