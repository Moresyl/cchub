import { memo } from "react";
import { ArrowRight, CheckCircle2, Globe2, MoonStar, Palette, ScanSearch, SunMedium, Wrench } from "lucide-react";
import type { Theme } from "../lib/theme";
import type { Locale } from "../lib/i18n";

interface WelcomeDialogProps {
  open: boolean;
  locale: Locale;
  theme: Theme;
  installedToolCount: number;
  profileCount: number;
  onSelectLocale: (locale: Locale) => void;
  onSelectTheme: (theme: Theme) => void;
  onFinish: () => void;
}

function uiText(locale: Locale, zhText: string, enText: string, jaText?: string) {
  return locale === "zh" ? zhText : locale === "ja" ? (jaText ?? enText) : enText;
}

function WelcomeDialogComponent({
  open,
  locale,
  theme,
  installedToolCount,
  profileCount,
  onSelectLocale,
  onSelectTheme,
  onFinish,
}: WelcomeDialogProps) {
  if (!open) {
    return null;
  }

  const localeOptions: Array<{ id: Locale; label: string }> = [
    { id: "zh", label: "中文" },
    { id: "en", label: "English" },
    { id: "ja", label: "日本語" },
  ];

  const themeOptions: Array<{ id: Theme; label: string; icon: typeof MoonStar }> = [
    { id: "dark", label: uiText(locale, "深色", "Dark", "ダーク"), icon: MoonStar },
    { id: "light", label: uiText(locale, "浅色", "Light", "ライト"), icon: SunMedium },
  ];

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 80,
        background: "rgba(10, 15, 28, 0.56)",
        backdropFilter: "blur(10px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 20,
      }}
    >
      <div
        className="card"
        style={{
          width: "min(760px, 100%)",
          padding: 24,
          borderRadius: 20,
          background: "linear-gradient(180deg, color-mix(in srgb, var(--bg-card) 94%, #dbeafe 6%), var(--bg-card))",
          border: "1px solid color-mix(in srgb, var(--accent) 16%, var(--border-default))",
          boxShadow: "0 28px 80px rgba(0, 0, 0, 0.28)",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", gap: 16, flexWrap: "wrap", alignItems: "flex-start" }}>
          <div>
            <div
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 8,
                padding: "6px 10px",
                borderRadius: 999,
                background: "color-mix(in srgb, var(--accent) 10%, transparent)",
                color: "var(--accent)",
                fontSize: 12,
                fontWeight: 700,
                marginBottom: 12,
              }}
            >
              <CheckCircle2 size={14} />
              {uiText(locale, "首次运行引导", "First Run Setup", "初回セットアップ")}
            </div>
            <h2 style={{ margin: 0, fontSize: 26, lineHeight: 1.15 }}>
              {uiText(locale, "欢迎使用 CCHub", "Welcome to CCHub", "CCHub へようこそ")}
            </h2>
            <p style={{ margin: "10px 0 0", color: "var(--text-secondary)", fontSize: 13, lineHeight: 1.7, maxWidth: 520 }}>
              {uiText(
                locale,
                "先完成语言、主题和现有配置扫描确认，之后你可以直接开始切换 Provider、管理 Session 与技能。",
                "Pick your language, theme, and confirm the existing config scan. After that, you can start managing providers, sessions, and skills immediately.",
                "言語とテーマを選び、既存設定のスキャン結果を確認してください。その後すぐに Provider、Session、スキル管理を始められます。",
              )}
            </p>
          </div>
          <div style={{ display: "grid", gap: 8, minWidth: 180 }}>
            <div className="badge badge-accent" style={{ justifyContent: "flex-start", padding: "8px 10px", fontSize: 11 }}>
              <Wrench size={12} />
              {uiText(locale, `已检测 ${installedToolCount} 个工具`, `${installedToolCount} tools detected`, `${installedToolCount} 個のツールを検出`)}
            </div>
            <div className="badge badge-muted" style={{ justifyContent: "flex-start", padding: "8px 10px", fontSize: 11 }}>
              <ScanSearch size={12} />
              {uiText(locale, `已发现 ${profileCount} 个配置`, `${profileCount} profiles found`, `${profileCount} 個のプロファイルを検出`)}
            </div>
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 16, marginTop: 24 }}>
          <section className="section-card" style={{ padding: 16 }}>
            <div className="section-card-title" style={{ marginBottom: 12 }}>
              <Globe2 size={16} />
              {uiText(locale, "步骤 1: 语言", "Step 1: Language", "ステップ 1: 言語")}
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {localeOptions.map((option) => (
                <button
                  key={option.id}
                  className={`btn btn-sm ${locale === option.id ? "btn-primary" : "btn-secondary"}`}
                  onClick={() => onSelectLocale(option.id)}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </section>

          <section className="section-card" style={{ padding: 16 }}>
            <div className="section-card-title" style={{ marginBottom: 12 }}>
              <Palette size={16} />
              {uiText(locale, "步骤 2: 主题", "Step 2: Theme", "ステップ 2: テーマ")}
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              {themeOptions.map((option) => {
                const Icon = option.icon;
                return (
                  <button
                    key={option.id}
                    className={`btn btn-sm ${theme === option.id ? "btn-primary" : "btn-secondary"}`}
                    onClick={() => onSelectTheme(option.id)}
                    style={{ gap: 6 }}
                  >
                    <Icon size={13} />
                    {option.label}
                  </button>
                );
              })}
            </div>
          </section>
        </div>

        <section className="section-card" style={{ marginTop: 16, padding: 16 }}>
          <div className="section-card-title" style={{ marginBottom: 8 }}>
            <ScanSearch size={16} />
            {uiText(locale, "步骤 3: 扫描现有配置", "Step 3: Scan Existing Config", "ステップ 3: 既存設定のスキャン")}
          </div>
          <p style={{ fontSize: 13, color: "var(--text-secondary)", margin: 0, lineHeight: 1.7 }}>
            {uiText(
              locale,
              profileCount > 0
                ? `已根据当前配置目录与数据库发现 ${profileCount} 个可用 Profile。后续你仍可在“配置切换”和“设置”页继续导入、同步或修复。`
                : "当前还没有发现可用 Profile。完成后可以在“配置切换”页创建官方 Provider，或在“设置”页执行迁移/修复。",
              profileCount > 0
                ? `${profileCount} existing profile(s) were found from your current config roots and local database. You can continue importing, syncing, or repairing later from Profiles and Settings.`
                : "No profiles were found yet. After this, create an official provider from Profiles or run migration and repair actions from Settings.",
              profileCount > 0
                ? `現在の設定ディレクトリとローカル DB から ${profileCount} 件の Profile を検出しました。後から Profiles と Settings でインポート、同期、修復を続けられます。`
                : "まだ Profile は見つかっていません。完了後に Profiles で公式 Provider を作成するか、Settings で移行や修復を実行してください。",
            )}
          </p>
        </section>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 20 }}>
          <button
            className="btn btn-primary"
            onClick={onFinish}
            style={{
              padding: "10px 28px",
              fontSize: 14,
              fontWeight: 600,
              borderRadius: 10,
              gap: 8,
              letterSpacing: 0.2,
              boxShadow: "0 2px 12px color-mix(in srgb, var(--accent) 36%, transparent)",
            }}
          >
            {uiText(locale, "开始使用", "Start Using CCHub", "使い始める")}
            <ArrowRight size={15} />
          </button>
        </div>
      </div>
    </div>
  );
}

export default memo(WelcomeDialogComponent);
