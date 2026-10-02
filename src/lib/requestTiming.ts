export interface RequestTimingRecord {
  is_streaming: boolean;
  status_code: number;
  latency_ms: number;
  output_tokens: number;
  first_output_ms?: number | null;
  generation_ms?: number | null;
}

export function getRequestTiming(record: RequestTimingRecord) {
  const first = record.first_output_ms;
  const validFirst =
    record.is_streaming &&
    typeof first === "number" &&
    Number.isSafeInteger(first) &&
    first >= 0 &&
    Number.isSafeInteger(record.latency_ms) &&
    first <= record.latency_ms;
  const firstOutputMs = validFirst ? first : null;
  const window = record.generation_ms;
  const validWindow =
    firstOutputMs != null &&
    typeof window === "number" &&
    Number.isSafeInteger(window) &&
    window >= 100 &&
    window <= record.latency_ms - firstOutputMs;
  const rate =
    validWindow &&
    record.status_code >= 200 &&
    record.status_code < 300 &&
    Number.isSafeInteger(record.output_tokens) &&
    record.output_tokens > 0
      ? (record.output_tokens * 1000) / window
      : null;
  return { firstOutputMs, tokensPerSecond: rate != null && Number.isFinite(rate) ? rate : null };
}

export function requestTimingLabel(
  record: RequestTimingRecord,
  text: (zh: string, en: string, ja?: string) => string,
): string | undefined {
  const timing = getRequestTiming(record);
  if (timing.firstOutputMs == null) return undefined;
  return [
    `${text("首个输出", "First output", "最初の出力")} ${timing.firstOutputMs} ms`,
    timing.tokensPerSecond == null ? null : `~${timing.tokensPerSecond.toFixed(1)} tok/s`,
  ]
    .filter(Boolean)
    .join(" · ");
}
