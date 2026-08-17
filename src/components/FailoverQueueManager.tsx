import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowDown, ArrowUp, Check, Loader2 } from "lucide-react";
import { getLocale } from "../lib/i18n";
import { showToast } from "./Toast";

type AppType = "claude" | "codex" | "gemini" | "grokbuild" | "opencode" | "openclaw" | "hermes";
interface QueueItem {
  providerId: string;
  providerName: string;
  priority: number;
  enabled: boolean;
}

export default function FailoverQueueManager({ appType }: { appType?: AppType }) {
  const locale = getLocale();
  const [selectedApp, setSelectedApp] = useState<AppType>(appType ?? "claude");
  const [items, setItems] = useState<QueueItem[]>([]);
  const [enabled, setEnabled] = useState(true);
  const [loading, setLoading] = useState(false);
  const text = useCallback((zh: string, en: string) => (locale === "zh" ? zh : en), [locale]);
  const appOptions = useMemo(
    () =>
      [
        ["claude", "Claude"],
        ["codex", "Codex"],
        ["gemini", "Gemini"],
        ["grokbuild", "Grok Build"],
        ["opencode", "OpenCode"],
        ["openclaw", "OpenClaw"],
        ["hermes", "Hermes"],
      ] as const,
    [],
  );

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [queue, autoFailover] = await Promise.all([
        invoke<QueueItem[]>("get_failover_queue", { appType: selectedApp }),
        invoke<boolean>("get_auto_failover_enabled"),
      ]);
      setItems(queue);
      setEnabled(autoFailover);
    } catch (error) {
      showToast("error", `${text("读取故障转移队列失败", "Failed to load failover queue")}: ${error}`);
    } finally {
      setLoading(false);
    }
  }, [selectedApp, text]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const persist = async (next: QueueItem[]) => {
    setItems(next);
    try {
      await invoke("set_failover_queue", { appType: selectedApp, providerIds: next.map((item) => item.providerId) });
    } catch (error) {
      showToast("error", `${text("保存故障转移顺序失败", "Failed to save failover order")}: ${error}`);
      void refresh();
    }
  };

  const toggle = async () => {
    const next = !enabled;
    setEnabled(next);
    try {
      await invoke("set_auto_failover_enabled", { enabled: next });
    } catch (error) {
      setEnabled(!next);
      showToast("error", `${text("保存自动故障转移设置失败", "Failed to save automatic failover setting")}: ${error}`);
    }
  };

  const move = (index: number, offset: -1 | 1) => {
    const target = index + offset;
    if (target < 0 || target >= items.length) return;
    const next = [...items];
    [next[index], next[target]] = [next[target], next[index]];
    void persist(next);
  };

  return (
    <div style={{ borderTop: "1px solid var(--border-default)", paddingTop: 16 }}>
      <div
        style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 10 }}
      >
        <div>
          <div
            style={{
              fontSize: 12,
              fontWeight: 700,
              color: "var(--text-muted)",
              textTransform: "uppercase",
              letterSpacing: 1,
            }}
          >
            {text("故障转移队列", "Failover Queue")}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 4 }}>
            {text(
              "按优先级切换供应商，自动跳过熔断中的 Profile。",
              "Switch providers by priority and skip profiles with an open circuit.",
            )}
          </div>
        </div>
        <button
          className="btn btn-ghost btn-sm"
          onClick={() => void toggle()}
          disabled={loading}
          aria-pressed={enabled}
        >
          <Check size={13} /> {enabled ? text("已启用", "Enabled") : text("已停用", "Disabled")}
        </button>
      </div>
      <select
        className="input input-sm"
        value={selectedApp}
        onChange={(event) => setSelectedApp(event.target.value as AppType)}
        disabled={loading}
        style={{ marginBottom: 10 }}
      >
        {appOptions.map(([id, label]) => (
          <option key={id} value={id}>
            {label}
          </option>
        ))}
      </select>
      {loading ? (
        <Loader2 size={15} className="animate-spin" />
      ) : items.length === 0 ? (
        <div style={{ color: "var(--text-muted)", fontSize: 12 }}>
          {text("当前应用没有可用 Profile。", "No provider profiles are available for this app.")}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {items.map((item, index) => (
            <div
              key={item.providerId}
              style={{
                display: "grid",
                gridTemplateColumns: "28px minmax(0, 1fr) auto",
                alignItems: "center",
                gap: 8,
                padding: "6px 8px",
                border: "1px solid var(--border-default)",
                borderRadius: 8,
              }}
            >
              <span style={{ color: "var(--text-muted)", fontSize: 11, textAlign: "center" }}>{index + 1}</span>
              <span
                style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 12 }}
                title={item.providerId}
              >
                {item.providerName}
              </span>
              <span style={{ display: "flex", gap: 3 }}>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => move(index, -1)}
                  disabled={index === 0 || loading}
                  aria-label={text("上移", "Move up")}
                >
                  <ArrowUp size={12} />
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => move(index, 1)}
                  disabled={index === items.length - 1 || loading}
                  aria-label={text("下移", "Move down")}
                >
                  <ArrowDown size={12} />
                </button>
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
