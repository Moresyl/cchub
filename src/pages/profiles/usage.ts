import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../../components/Toast";
import type { ConfigProfile } from "./helpers";

interface UsageQueryResult {
  success?: boolean;
  data?: unknown;
  error?: string;
}

function summarizeUsage(value: unknown) {
  const first = Array.isArray(value) ? value[0] : value;
  if (!first || typeof first !== "object") return String(first ?? "OK");
  return Object.entries(first as Record<string, unknown>)
    .filter(([key]) => ["planName", "remaining", "used", "total", "unit", "resetAt"].includes(key))
    .map(([key, item]) => `${key}: ${String(item)}`)
    .join(" · ");
}

export async function queryProfileUsage(profile: ConfigProfile, locale: string) {
  const result = await invoke<UsageQueryResult>("queryProviderUsage", {
    providerId: profile.id,
    app: profile.tool_id,
  });
  if (!result.success || result.error) {
    showToast(
      "error",
      result.error ||
        (locale === "zh" ? "该 Provider 暂无可用用量接口" : "No usage endpoint is available for this provider"),
    );
    return;
  }
  showToast("success", `${profile.name}: ${summarizeUsage(result.data) || "OK"}`);
}

export function startProfileUsageQuery(profile: ConfigProfile, locale: string) {
  void queryProfileUsage(profile, locale).catch((error) =>
    showToast("error", locale === "zh" ? `用量查询失败: ${error}` : `Usage query failed: ${error}`),
  );
}
