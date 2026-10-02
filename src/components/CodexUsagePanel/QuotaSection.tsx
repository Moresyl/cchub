import { useId } from "react";
import type { CliQuota, Resource } from "./useCliUsage";

export type LocaleText = (zh: string, en: string, ja?: string) => string;

export default function QuotaSection({
  tool,
  resource,
  text,
}: {
  tool: "Codex" | "Claude";
  resource: Resource<CliQuota>;
  text: LocaleText;
}) {
  const heading = useId();
  const quota = resource.data;
  const status = quota?.credentialStatus;
  const labels: Record<string, string> = {
    five_hour: text("5 小时窗口", "5-hour window", "5時間ウィンドウ"),
    seven_day: text("7 天窗口", "7-day window", "7日間ウィンドウ"),
    thirty_day: text("30 天窗口", "30-day window", "30日間ウィンドウ"),
  };
  const unavailable =
    status === "not_found"
      ? text(
          `未检测到 ${tool} OAuth 登录。`,
          `No ${tool} OAuth login detected.`,
          `${tool} OAuth ログインが検出されませんでした。`,
        )
      : status === "expired"
        ? text(
            `认证已失效，请在 ${tool} CLI 中重新登录。`,
            `Sign in again with the ${tool} CLI.`,
            `${tool} CLI で再ログインしてください。`,
          )
        : status === "parse_error"
          ? text(
              "无法读取本机凭据，请检查配置目录或重新登录。",
              "Cannot read local credentials. Check the configured directory or sign in again.",
              "ローカル資格情報を読み込めません。設定ディレクトリを確認するか再ログインしてください。",
            )
          : text(
              "暂时无法查询，请检查网络后刷新。",
              "Temporarily unavailable. Check your connection and refresh.",
              "照会できません。接続を確認して更新してください。",
            );
  return (
    <section aria-labelledby={heading} className="min-w-0 space-y-3">
      <h4 id={heading} className="text-xs font-[590]">
        {tool} {text("订阅用量", "subscription usage", "利用量")}
      </h4>
      {resource.status === "loading" && (
        <p role="status" className="text-muted-foreground">
          {text("正在查询…", "Querying…", "照会中…")}
        </p>
      )}
      {resource.status === "error" && (
        <p role="alert" className="text-[var(--warning)]">
          {text(
            "查询失败，请检查网络或本机登录状态后刷新。",
            "Query failed. Check your connection or local sign-in and refresh.",
            "照会に失敗しました。接続とログイン状態を確認して更新してください。",
          )}
        </p>
      )}
      {quota && resource.status !== "ready" && (
        <p className="text-muted-foreground">
          {text("以下为上次读取结果。", "Last retrieved result below.", "以下は前回の取得結果です。")}
        </p>
      )}
      {quota &&
        (!quota.success ? (
          <p className="text-muted-foreground">{unavailable}</p>
        ) : quota.tiers.length === 0 ? (
          <p className="text-muted-foreground">
            {text(
              "已登录，服务端未返回用量窗口。",
              "Signed in; no usage windows reported.",
              "ログイン済みですが利用量ウィンドウがありません。",
            )}
          </p>
        ) : (
          <div className="grid gap-3">
            {quota.tiers.map((tier, index) => {
              const name = labels[tier.name] ?? tier.name;
              const used = tier.utilization;
              return (
                <div key={`${tier.name}:${index}`} className="space-y-1.5">
                  <div className="flex items-start justify-between gap-3">
                    <span className="min-w-0 break-words">{name}</span>
                    <span className="shrink-0 tabular-nums text-muted-foreground">{used.toFixed(1)}%</span>
                  </div>
                  <div
                    role="progressbar"
                    aria-label={`${tool} ${name}`}
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={used}
                    className="h-1 overflow-hidden rounded-sm bg-[var(--border-default)]"
                  >
                    <div
                      className="h-full"
                      style={{
                        width: `${used}%`,
                        background:
                          used >= 90 ? "var(--danger)" : used >= 70 ? "var(--warning)" : "var(--text-secondary)",
                      }}
                    />
                  </div>
                  {tier.resetsAt && (
                    <p className="break-all text-muted-foreground">
                      {text("重置", "Reset", "リセット")}: {tier.resetsAt}
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        ))}
    </section>
  );
}
