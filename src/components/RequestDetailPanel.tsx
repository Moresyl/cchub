import { memo } from "react";
import CollapsibleSection from "./CollapsibleSection";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

export interface StreamAttemptRecord {
  attempt_id: string;
  profile_id: string;
  provider_name: string;
  model: string | null;
  response_model: string | null;
  status_code: number;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  total_cost_usd: string;
}

export interface RequestDetailRecord {
  request_id: string;
  tool_id: string;
  profile_id: string;
  provider_name: string;
  request_model: string | null;
  response_model: string | null;
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_creation_tokens: number;
  total_cost_usd: string;
  latency_ms: number;
  status_code: number;
  is_streaming: boolean;
  error_message: string | null;
  created_at: string;
  stream_attempts?: StreamAttemptRecord[];
}

interface Props {
  record: RequestDetailRecord | null;
  loading: boolean;
  title: string;
  closeLabel: string;
  onClose: () => void;
  error?: string | null;
  onRetry?: () => void;
  localeText?: (zh: string, en: string, ja?: string) => string;
}

const englishText = (_zh: string, en: string) => en;

function RequestDetailPanel({
  record,
  loading,
  title,
  closeLabel,
  onClose,
  error,
  onRetry,
  localeText: text = englishText,
}: Props) {
  if (!record && !loading && !error) return null;
  return (
    <Card className="mt-3 min-w-0" aria-busy={loading}>
      <CardHeader className="flex-row flex-wrap items-center justify-between gap-2">
        <CardTitle>{title}</CardTitle>
        <Button variant="ghost" type="button" onClick={onClose}>
          {closeLabel}
        </Button>
      </CardHeader>
      <CardContent className="space-y-4">
        {loading ? (
          <p role="status" className="text-xs text-muted-foreground">
            {text("正在加载请求明细…", "Loading request details…")}
          </p>
        ) : error ? (
          <div role="alert" className="space-y-3 text-xs">
            <p className="break-words text-[var(--danger)]">{error}</p>
            {onRetry && (
              <Button variant="secondary" onClick={onRetry}>
                {text("重试", "Retry")}
              </Button>
            )}
          </div>
        ) : record ? (
          <>
            <dl className="grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
              <Detail label={text("请求 ID", "Request ID")} value={record.request_id} />
              <Detail label={text("供应商", "Provider")} value={record.provider_name} />
              <Detail label={text("状态", "Status")} value={String(record.status_code)} />
              <Detail label={text("延迟", "Latency")} value={`${record.latency_ms} ms`} />
              <Detail label={text("估算费用", "Estimated cost")} value={`$${record.total_cost_usd}`} />
              <Detail
                label={text("输入 / 输出 Token", "Input / output tokens")}
                value={`${record.input_tokens} / ${record.output_tokens}`}
              />
              <Detail
                label={text("缓存读取 / 写入", "Cache read / write")}
                value={`${record.cache_read_tokens} / ${record.cache_creation_tokens}`}
              />
              <Detail
                label={text("请求 / 响应模型", "Request / response model")}
                value={`${record.request_model ?? "—"} / ${record.response_model ?? "—"}`}
              />
              <Detail
                label={text("响应方式", "Response mode")}
                value={record.is_streaming ? text("流式", "Streaming") : text("普通", "Standard")}
              />
              {record.error_message && <Detail label={text("错误", "Error")} value={record.error_message} />}
            </dl>
            {!!record.stream_attempts?.length && (
              <CollapsibleSection
                key={record.request_id}
                title={text("流式失败尝试", "Failed stream attempts")}
                summary={text(`${record.stream_attempts.length} 次`, `${record.stream_attempts.length} attempts`)}
              >
                <p className="mb-3 text-xs leading-relaxed text-muted-foreground">
                  {text(
                    "各次失败的用量独立保留。上方显示最终响应的数据，下方尝试未合并到用量汇总。",
                    "Usage from failed attempts is retained separately. The data above and usage summaries describe the final response and do not include these attempts.",
                  )}
                </p>
                <ol className="max-h-[320px] space-y-3 overflow-y-auto pr-1">
                  {record.stream_attempts.map((attempt, index) => (
                    <li key={attempt.attempt_id} className="rounded-md border border-border p-3">
                      <p className="mb-3 break-words text-xs font-medium">
                        {index + 1}. {attempt.provider_name} · HTTP {attempt.status_code}
                      </p>
                      <dl className="grid min-w-0 grid-cols-1 gap-3 sm:grid-cols-2">
                        <Detail label={text("模型", "Model")} value={attempt.response_model ?? attempt.model ?? "—"} />
                        <Detail
                          label={text("输入 / 输出 Token", "Input / output tokens")}
                          value={`${attempt.input_tokens} / ${attempt.output_tokens}`}
                        />
                        <Detail
                          label={text("缓存读取 / 写入", "Cache read / write")}
                          value={`${attempt.cache_read_tokens} / ${attempt.cache_creation_tokens}`}
                        />
                        <Detail label={text("估算费用", "Estimated cost")} value={`$${attempt.total_cost_usd}`} />
                      </dl>
                    </li>
                  ))}
                </ol>
              </CollapsibleSection>
            )}
          </>
        ) : null}
      </CardContent>
    </Card>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 space-y-1">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="break-words text-xs leading-relaxed [overflow-wrap:anywhere]">{value}</dd>
    </div>
  );
}

export default memo(RequestDetailPanel);
