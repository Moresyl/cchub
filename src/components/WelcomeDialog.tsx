import { memo } from "react";
import {
  ArrowRight,
  CheckCircle2,
  Globe2,
  Monitor,
  MoonStar,
  Palette,
  ScanSearch,
  SunMedium,
  Wrench,
} from "lucide-react";
import type { Theme } from "../lib/theme";
import type { Locale } from "../lib/i18n";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

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
  const localeOptions: Array<{ id: Locale; label: string }> = [
    { id: "zh", label: "中文" },
    { id: "en", label: "English" },
    { id: "ja", label: "日本語" },
  ];
  const themeOptions: Array<{ id: Theme; label: string; icon: typeof MoonStar }> = [
    { id: "dark", label: uiText(locale, "深色", "Dark", "ダーク"), icon: MoonStar },
    { id: "light", label: uiText(locale, "浅色", "Light", "ライト"), icon: SunMedium },
    { id: "system", label: uiText(locale, "跟随系统", "System", "システムに合わせる"), icon: Monitor },
  ];

  const scanDescription = uiText(
    locale,
    profileCount > 0
      ? `已根据当前配置目录与数据库发现 ${profileCount} 个可用 Profile。后续仍可在“配置切换”和“设置”中继续导入、同步或修复。`
      : "当前还没有发现可用 Profile。完成后可在“配置切换”中创建官方 Provider，或在“设置”中执行迁移与修复。",
    profileCount > 0
      ? `${profileCount} existing profile(s) were found. Continue importing, syncing, or repairing later from Profiles and Settings.`
      : "No profiles were found yet. Create an official provider from Profiles or run migration and repair actions from Settings.",
    profileCount > 0
      ? `${profileCount} 件の Profile を検出しました。後から Profiles と Settings でインポート、同期、修復を続けられます。`
      : "まだ Profile は見つかっていません。Profiles で公式 Provider を作成するか、Settings で移行や修復を実行してください。",
  );

  return (
    <Dialog open={open}>
      <DialogContent hideClose className="max-w-[760px]">
        <DialogHeader>
          <div className="grid size-9 shrink-0 place-items-center rounded-[7px] bg-[var(--accent-subtle)] text-primary">
            <CheckCircle2 size={17} aria-hidden="true" />
          </div>
          <div className="min-w-0 flex-1">
            <DialogTitle>{uiText(locale, "欢迎使用 CCHub", "Welcome to CCHub", "CCHub へようこそ")}</DialogTitle>
            <DialogDescription>
              {uiText(
                locale,
                "完成语言、主题与现有配置确认，然后即可管理 Provider、Session 与技能。",
                "Confirm language, theme, and the existing configuration scan, then start managing providers, sessions, and skills.",
                "言語、テーマ、既存設定の確認後、Provider、Session、スキル管理を開始できます。",
              )}
            </DialogDescription>
          </div>
          <div className="hidden gap-1.5 sm:flex">
            <span className="badge badge-accent">
              <Wrench size={11} aria-hidden="true" />
              {installedToolCount}
            </span>
            <span className="badge badge-muted">
              <ScanSearch size={11} aria-hidden="true" />
              {profileCount}
            </span>
          </div>
        </DialogHeader>

        <DialogBody className="space-y-3">
          <div className="grid gap-3 sm:grid-cols-2">
            <Card>
              <CardHeader className="pb-2.5">
                <CardTitle className="flex items-center gap-2 text-xs">
                  <Globe2 size={15} aria-hidden="true" />
                  {uiText(locale, "1. 语言", "1. Language", "1. 言語")}
                </CardTitle>
                <CardDescription>
                  {uiText(locale, "选择界面显示语言", "Choose the interface language", "表示言語を選択")}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-2">
                {localeOptions.map((option) => (
                  <Button
                    key={option.id}
                    variant={locale === option.id ? "default" : "secondary"}
                    size="sm"
                    aria-pressed={locale === option.id}
                    onClick={() => onSelectLocale(option.id)}
                  >
                    {option.label}
                  </Button>
                ))}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2.5">
                <CardTitle className="flex items-center gap-2 text-xs">
                  <Palette size={15} aria-hidden="true" />
                  {uiText(locale, "2. 主题", "2. Theme", "2. テーマ")}
                </CardTitle>
                <CardDescription>
                  {uiText(locale, "选择明暗外观", "Choose a light or dark appearance", "明暗テーマを選択")}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-2">
                {themeOptions.map((option) => {
                  const Icon = option.icon;
                  return (
                    <Button
                      key={option.id}
                      variant={theme === option.id ? "default" : "secondary"}
                      size="sm"
                      aria-pressed={theme === option.id}
                      onClick={() => onSelectTheme(option.id)}
                    >
                      <Icon size={13} aria-hidden="true" />
                      {option.label}
                    </Button>
                  );
                })}
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader className="pb-2.5">
              <CardTitle className="flex items-center gap-2 text-xs">
                <ScanSearch size={15} aria-hidden="true" />
                {uiText(locale, "3. 现有配置", "3. Existing configuration", "3. 既存設定")}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-xs leading-relaxed text-muted-foreground">{scanDescription}</p>
            </CardContent>
          </Card>
        </DialogBody>

        <DialogFooter>
          <Button onClick={onFinish}>
            {uiText(locale, "开始使用", "Start Using CCHub", "使い始める")}
            <ArrowRight size={14} aria-hidden="true" />
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export default memo(WelcomeDialogComponent);
