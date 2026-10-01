import { useId, useState } from "react";
import { BarChart3, ChevronDown, Loader2, RefreshCw } from "lucide-react";
import { useCopilotAccountResources } from "../hooks/useCopilotAccountResources";
import type { CopilotResourceFailure } from "../lib/copilotAccounts";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { SimpleSelect } from "./ui/simple-select";
import { QuotaItem } from "./copilotUsage/QuotaItem";
import ModelsList from "./copilotUsage/ModelsList";

type LocaleText = (zh: string, en: string, ja?: string) => string;

function failureText(reason: CopilotResourceFailure, text: LocaleText) {
  switch (reason) {
    case "sign_in_required":
      return text(
        "认证已失效，请重新登录此账号。",
        "Sign in to this account again.",
        "このアカウントに再ログインしてください。",
      );
    case "subscription_unavailable":
      return text(
        "此账号的订阅暂不可用或没有查询权限。",
        "The subscription or query permission is unavailable.",
        "サブスクリプションまたは照会権限が利用できません。",
      );
    case "rate_limited":
      return text(
        "查询过于频繁，请稍后刷新。",
        "Rate limited. Refresh later.",
        "照会制限に達しました。後で更新してください。",
      );
    case "invalid_response":
      return text(
        "返回数据不完整或格式异常，请稍后重试。",
        "The response is incomplete or invalid. Retry later.",
        "応答データが不完全または無効です。後で再試行してください。",
      );
    case "timeout":
      return text(
        "查询超时，请检查网络后重试。",
        "The query timed out. Check your connection and retry.",
        "照会がタイムアウトしました。接続を確認して再試行してください。",
      );
    default:
      return text(
        "暂时无法查询，请检查网络后重试。",
        "Temporarily unavailable. Check your connection and retry.",
        "一時的に照会できません。接続を確認して再試行してください。",
      );
  }
}

export default function CopilotUsagePanel({ localeText: text }: { localeText: LocaleText }) {
  const { auth, selection, account, data, loading, listing, error, refresh, select } = useCopilotAccountResources();
  const [expanded, setExpanded] = useState(false);
  const headingId = useId();
  const modelsId = useId();
  const accounts = auth?.accounts ?? [];
  const defaultAccount = accounts.find((item) => item.id === auth?.default_account_id) ?? accounts[0];
  const usage = data?.usage;
  const options = [
    {
      value: "",
      label: defaultAccount
        ? text(
            `默认账号 · ${defaultAccount.login}`,
            `Default · ${defaultAccount.login}`,
            `既定 · ${defaultAccount.login}`,
          )
        : text("默认账号", "Default account", "既定のアカウント"),
    },
    ...accounts.map((item) => ({ value: item.id, label: item.login })),
  ];
  const checkedAt = data ? new Date(data.fetched_at) : null;
  return (
    <Card className="mt-3 min-w-0" aria-labelledby={headingId} aria-busy={loading}>
      <CardHeader className="gap-3">
        <div className="flex items-start justify-between gap-3">
          <CardTitle id={headingId} className="flex items-center gap-2 text-[14px] leading-5">
            <BarChart3 size={16} className="shrink-0 text-muted-foreground" aria-hidden="true" />
            {text("Copilot 配额与模型", "Copilot quota and models", "Copilot クォータとモデル")}
          </CardTitle>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={text(
              "刷新 Copilot 配额与模型",
              "Refresh Copilot quota and models",
              "Copilot クォータとモデルを更新",
            )}
            disabled={loading}
            onClick={() => void refresh()}
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
            "按账号查看订阅额度和可用模型，查询不会切换默认账号。",
            "Check each account's quota and models without changing the default account.",
            "既定のアカウントを変更せず、各アカウントのクォータとモデルを確認します。",
          )}
        </CardDescription>
        {accounts.length > 0 && (
          <SimpleSelect
            value={selection}
            options={options}
            onValueChange={select}
            disabled={listing}
            ariaLabel={text("查询 Copilot 账号", "Copilot account to query", "照会する Copilot アカウント")}
            className="w-full min-w-0"
          />
        )}
      </CardHeader>
      <CardContent className="grid gap-4 text-[12px]">
        {loading ? (
          <p role="status" className="text-muted-foreground">
            {text("正在查询所选账号…", "Querying the selected account…", "選択したアカウントを照会中…")}
          </p>
        ) : error ? (
          <p role="alert" className="text-muted-foreground">
            {text(
              "查询失败或账号已变更，请刷新重试。",
              "The query failed or the account changed. Refresh to retry.",
              "照会に失敗したかアカウントが変更されました。更新して再試行してください。",
            )}
          </p>
        ) : !account ? (
          <p className="text-muted-foreground">
            {text(
              "登录 Copilot 后可查询配额。",
              "Sign in to Copilot to query quota.",
              "Copilot にログインするとクォータを確認できます。",
            )}
          </p>
        ) : data ? (
          <>
            <div className="flex flex-wrap items-center justify-between gap-2 text-muted-foreground">
              <span className="min-w-0 break-all">
                {text("当前查询", "Current account", "照会中")} · {data.account.login}
              </span>
              {checkedAt && Number.isFinite(checkedAt.getTime()) && (
                <time dateTime={data.fetched_at}>
                  {text("更新于", "Updated", "更新")}{" "}
                  {checkedAt.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                </time>
              )}
            </div>
            {data.usage_error ? (
              <p role="alert" className="text-muted-foreground">
                {text("配额", "Quota", "クォータ")}: {failureText(data.usage_error, text)}
              </p>
            ) : (
              usage && (
                <>
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="badge badge-muted break-all">{usage.copilot_plan}</span>
                    {usage.quota_reset_date && (
                      <span className="break-all text-muted-foreground">
                        {text("重置", "Reset", "リセット")}: {usage.quota_reset_date}
                      </span>
                    )}
                  </div>
                  <div className="grid gap-4">
                    {(
                      [
                        [
                          text("Premium 请求", "Premium requests", "Premium リクエスト"),
                          usage.quota_snapshots.premium_interactions,
                        ],
                        [text("聊天", "Chat", "チャット"), usage.quota_snapshots.chat],
                        [text("补全", "Completions", "補完"), usage.quota_snapshots.completions],
                      ] as const
                    ).map(([label, value]) => (
                      <QuotaItem
                        key={label}
                        label={label}
                        value={value}
                        unknownLabel={text("未提供", "Not reported", "未報告")}
                        unlimitedLabel={text("不限量", "Unlimited", "無制限")}
                      />
                    ))}
                  </div>
                </>
              )
            )}
            <div className="grid min-w-0 gap-2 border-t border-border pt-3">
              {data.models_error ? (
                <p role="alert" className="text-muted-foreground">
                  {text("模型", "Models", "モデル")}: {failureText(data.models_error, text)}
                </p>
              ) : (
                data.models && (
                  <>
                    <Button
                      type="button"
                      variant="ghost"
                      className="w-full justify-between"
                      disabled={data.models.length === 0}
                      aria-expanded={expanded}
                      aria-controls={modelsId}
                      onClick={() => setExpanded((value) => !value)}
                    >
                      {text(
                        `可用模型 ${data.models.length} 个`,
                        `${data.models.length} models available`,
                        `${data.models.length} モデル利用可能`,
                      )}
                      <ChevronDown size={14} className={expanded ? "rotate-180" : ""} aria-hidden="true" />
                    </Button>
                    {expanded && data.models.length > 0 && (
                      <ModelsList
                        key={`${data.account.id}:${data.account.revision}`}
                        models={data.models}
                        id={modelsId}
                        text={text}
                      />
                    )}
                  </>
                )
              )}
            </div>
          </>
        ) : null}
      </CardContent>
    </Card>
  );
}
