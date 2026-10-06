import { useId } from "react";
import { BarChart3, Loader2, RefreshCw } from "lucide-react";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import QuotaSection, { type LocaleText } from "./CodexUsagePanel/QuotaSection";
import { useCliUsage } from "./CodexUsagePanel/useCliUsage";

export default function CodexUsagePanel({ localeText: text }: { localeText: LocaleText }) {
  const heading = useId();
  const { codex, claude, models, loading, refresh } = useCliUsage();
  return (
    <Card className="mt-3 min-w-0" aria-labelledby={heading}>
      <CardHeader className="gap-3">
        <div className="flex items-start justify-between gap-3">
          <CardTitle id={heading} className="flex items-center gap-2 text-sm leading-5">
            <BarChart3 size={16} className="shrink-0 text-muted-foreground" aria-hidden="true" />
            {text("本机 CLI 配额与模型", "Local CLI quota and models", "ローカル CLI クォータとモデル")}
          </CardTitle>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            disabled={loading}
            onClick={() => void refresh()}
            aria-label={text(
              "刷新本机配额与模型",
              "Refresh local quota and models",
              "ローカルのクォータとモデルを更新",
            )}
          >
            {loading ? (
              <Loader2 size={14} className="animate-spin motion-reduce:animate-none" aria-hidden="true" />
            ) : (
              <RefreshCw size={14} aria-hidden="true" />
            )}
          </Button>
        </div>
        <CardDescription>
          {text(
            "读取配置目录中的 CLI 登录；各项独立更新，查询不会切换账号。",
            "Read CLI sign-ins from configured directories. Each query updates independently without switching accounts.",
            "設定ディレクトリの CLI ログインを読み込みます。アカウントを切り替えず各項目を更新します。",
          )}
        </CardDescription>
      </CardHeader>
      <CardContent className="grid min-w-0 gap-5 text-xs">
        <div className="grid min-w-0 gap-5 sm:grid-cols-2">
          <QuotaSection tool="Codex" resource={codex} text={text} />
          <QuotaSection tool="Claude" resource={claude} text={text} />
        </div>
        <section
          className="min-w-0 space-y-2 border-t border-border pt-4"
          aria-label={text("Codex 模型目录", "Codex model catalog", "Codex モデル一覧")}
        >
          <h4 className="font-semibold">{text("Codex 模型目录", "Codex model catalog", "Codex モデル一覧")}</h4>
          {models.status === "loading" && (
            <p role="status" className="text-muted-foreground">
              {text("正在读取目录…", "Reading catalog…", "一覧を読み込み中…")}
            </p>
          )}
          {models.status === "error" && (
            <p role="alert" className="text-[var(--warning)]">
              {text(
                "模型目录读取失败，请检查本机登录状态和网络后刷新。",
                "Catalog query failed. Check local sign-in and connection, then refresh.",
                "モデル一覧の照会に失敗しました。ログイン状態と接続を確認して更新してください。",
              )}
            </p>
          )}
          {models.data && (
            <p className="text-muted-foreground">
              {models.status === "ready"
                ? text(
                    `目录返回 ${models.data.length} 个模型`,
                    `Catalog returned ${models.data.length} models`,
                    `一覧に ${models.data.length} モデル`,
                  )
                : text(
                    `上次读取的目录：${models.data.length} 个模型`,
                    `Last retrieved catalog: ${models.data.length} models`,
                    `前回取得した一覧：${models.data.length} モデル`,
                  )}
            </p>
          )}
        </section>
      </CardContent>
    </Card>
  );
}
