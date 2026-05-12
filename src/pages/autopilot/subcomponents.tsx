/* eslint-disable @typescript-eslint/no-explicit-any */
import { type ReactNode } from "react";
import { FolderOpen, Trash2 } from "lucide-react";

import type { LiveEvent } from "./helpers";

export function TaskFileList(props: {
  files: string[];
  label: string;
  description: string;
  addLabel: string;
  clearLabel: string;
  emptyLabel: string;
  onPick: () => void;
  onRemove: (idx: number) => void;
  onMove: (idx: number, direction: -1 | 1) => void;
  onClear: () => void;
}) {
  const { files } = props;
  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
        <label className="field-label" style={{ marginBottom: 0 }}>
          {props.label}
          {files.length > 0 && (
            <span style={{ marginLeft: 8, fontSize: 11, color: "var(--text-muted)", fontWeight: 400 }}>
              ({files.length})
            </span>
          )}
        </label>
        <div style={{ display: "flex", gap: 6 }}>
          {files.length > 0 && (
            <button className="btn btn-ghost btn-xs" onClick={props.onClear}>
              {props.clearLabel}
            </button>
          )}
          <button className="btn btn-secondary btn-sm" onClick={props.onPick} style={{ gap: 6 }}>
            <FolderOpen size={13} />
            {props.addLabel}
          </button>
        </div>
      </div>
      <p style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>{props.description}</p>
      {files.length === 0 ? (
        <div
          style={{
            padding: "14px 12px",
            border: "1px dashed var(--border-default)",
            borderRadius: 8,
            fontSize: 12,
            color: "var(--text-muted)",
            textAlign: "center",
          }}
        >
          {props.emptyLabel}
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {files.map((f, idx) => {
            const parts = f.split(/[\\/]/).filter(Boolean);
            const name = parts[parts.length - 1] || f;
            return (
              <div
                key={`${f}-${idx}`}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 8,
                  padding: "6px 10px",
                  borderRadius: 6,
                  background: "var(--bg-elevated)",
                  border: "1px solid var(--border-subtle)",
                }}
              >
                <span
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: "50%",
                    background: "var(--bg-app)",
                    display: "inline-flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: 11,
                    fontWeight: 600,
                    color: "var(--text-secondary)",
                    flexShrink: 0,
                  }}
                >
                  {idx + 1}
                </span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      fontSize: 13,
                      fontWeight: 500,
                      color: "var(--text-primary)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {name}
                  </div>
                  <div
                    style={{
                      fontSize: 10,
                      color: "var(--text-muted)",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {f}
                  </div>
                </div>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => props.onMove(idx, -1)}
                  disabled={idx === 0}
                  title="Up"
                >
                  ↑
                </button>
                <button
                  className="btn btn-ghost btn-icon-sm"
                  onClick={() => props.onMove(idx, 1)}
                  disabled={idx === files.length - 1}
                  title="Down"
                >
                  ↓
                </button>
                <button className="btn btn-ghost btn-icon-sm" onClick={() => props.onRemove(idx)} title="Remove">
                  <Trash2 size={13} />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export function PathField(props: {
  label: string;
  value: string;
  placeholder: string;
  buttonLabel: string;
  onChange: (value: string) => void;
  onPick: () => void;
}) {
  return (
    <div>
      <label className="field-label">{props.label}</label>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          className="input"
          value={props.value}
          onChange={(event) => props.onChange(event.target.value)}
          placeholder={props.placeholder}
          style={{ flex: 1 }}
        />
        <button className="btn btn-secondary btn-sm" onClick={props.onPick} style={{ gap: 6, flexShrink: 0 }}>
          <FolderOpen size={14} />
          {props.buttonLabel}
        </button>
      </div>
    </div>
  );
}

export function TextField(props: {
  label: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}) {
  return (
    <div>
      <label className="field-label">{props.label}</label>
      <input
        className="input"
        value={props.value}
        onChange={(event) => props.onChange(event.target.value)}
        placeholder={props.placeholder}
      />
    </div>
  );
}

export function ToggleOption(props: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "10px 12px",
        borderRadius: 10,
        border: "1px solid var(--border-default)",
        background: "var(--bg-card)",
        cursor: "pointer",
      }}
    >
      <input type="checkbox" checked={props.checked} onChange={(event) => props.onChange(event.target.checked)} />
      <span style={{ fontSize: 13, color: "var(--text-secondary)" }}>{props.label}</span>
    </label>
  );
}

export function MetricCard(props: { title: string; value: string; tone: string; icon: ReactNode }) {
  return (
    <div className="card" style={{ padding: 16, minWidth: 0 }}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center" }}>
        <div
          style={{
            fontSize: 12,
            color: "var(--text-muted)",
            minWidth: 0,
            flex: 1,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
          title={props.title}
        >
          {props.title}
        </div>
        <div style={{ color: props.tone }}>{props.icon}</div>
      </div>
      <div
        style={{
          marginTop: 10,
          fontSize: 18,
          fontWeight: 700,
          color: props.tone,
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={props.value}
      >
        {props.value}
      </div>
    </div>
  );
}

export function SummaryItem(props: { label: string; value: string; strong?: boolean }) {
  return (
    <div>
      <div className="field-label" style={{ marginBottom: 6 }}>
        {props.label}
      </div>
      <div
        style={{
          fontSize: props.strong ? 15 : 13,
          fontWeight: props.strong ? 600 : 400,
          color: props.strong ? "var(--text-primary)" : "var(--text-secondary)",
          lineHeight: 1.6,
        }}
      >
        {props.value}
      </div>
    </div>
  );
}

// 把 codex --json 的事件对象压成一行友好摘要。不认识的字段就回落到 JSON 单行。
export function summarizeCodexJson(value: unknown): { label: string; tone: string; detail: string } {
  const tone = "var(--text-secondary)";
  if (!value || typeof value !== "object") {
    return { label: "event", tone, detail: typeof value === "string" ? value : JSON.stringify(value) };
  }
  const obj = value as Record<string, unknown>;
  // codex v1: { type, msg: {...} } 或 { type, role, content }
  const type = typeof obj.type === "string" ? (obj.type as string) : "";
  const msg = (obj.msg as Record<string, unknown> | undefined) ?? undefined;
  const content = (obj.content as unknown) ?? (msg?.content as unknown);
  if (type.includes("delta") || type.includes("token")) {
    const text = typeof content === "string" ? content : ((msg?.text as string) ?? "");
    return { label: "delta", tone: "var(--text-primary)", detail: text };
  }
  if (type.includes("tool") && type.includes("call")) {
    const name = (msg?.name as string) ?? (obj.name as string) ?? "tool";
    return { label: `tool→${name}`, tone: "#2563eb", detail: JSON.stringify(obj.arguments ?? msg?.arguments ?? "") };
  }
  if (type.includes("tool") && (type.includes("result") || type.includes("output"))) {
    const out = JSON.stringify(obj.output ?? msg?.output ?? obj.result ?? msg?.result ?? "");
    return { label: "tool←", tone: "#059669", detail: out };
  }
  if (type.includes("turn") || type.includes("complete")) {
    return { label: type, tone: "#059669", detail: JSON.stringify(msg ?? obj) };
  }
  if (type.includes("error")) {
    return { label: type, tone: "#dc2626", detail: JSON.stringify(msg ?? obj) };
  }
  if (type) {
    return { label: type, tone, detail: JSON.stringify(msg ?? obj) };
  }
  // 无 type 字段：直接把整对象单行序列化
  return { label: "event", tone, detail: JSON.stringify(obj) };
}

export function LiveEventRow({
  event,
  uiText,
}: {
  event: LiveEvent;
  uiText: (zh: string, en: string, ja?: string) => string;
}) {
  const rowBase: React.CSSProperties = {
    display: "flex",
    gap: 8,
    padding: "3px 0",
    borderBottom: "1px dashed var(--border-subtle)",
    whiteSpace: "pre-wrap",
    wordBreak: "break-word",
  };
  const labelStyle: React.CSSProperties = {
    flexShrink: 0,
    minWidth: 72,
    fontSize: 10,
    fontWeight: 700,
    letterSpacing: "0.04em",
    textTransform: "uppercase",
    color: "var(--text-muted)",
    paddingTop: 1,
  };

  if (event.kind === "round-start") {
    const p = event.payload;
    const modeText =
      p.mode === "initial"
        ? uiText("启动新会话", "initial", "新規セッション")
        : uiText("续跑会话", "resume", "継続セッション");
    return (
      <div
        style={{
          ...rowBase,
          background: "var(--accent-subtle)",
          padding: "6px 8px",
          borderRadius: 6,
          borderBottom: "none",
          margin: "6px 0",
        }}
      >
        <span style={{ ...labelStyle, color: "var(--accent)" }}>ROUND {p.attempt}</span>
        <span style={{ color: "var(--text-primary)", fontWeight: 600 }}>
          {uiText(
            `第 ${p.attempt} 轮开始（${modeText}）`,
            `Round ${p.attempt} started (${modeText})`,
            `ラウンド ${p.attempt} 開始（${modeText}）`,
          )}
        </span>
      </div>
    );
  }

  if (event.kind === "round-end") {
    const p = event.payload;
    const sec = Math.floor(p.elapsed_ms / 1000);
    const ok = p.return_code === 0;
    return (
      <div
        style={{
          ...rowBase,
          padding: "6px 8px",
          borderRadius: 6,
          borderBottom: "none",
          margin: "6px 0",
          background: ok ? "rgba(5, 150, 105, 0.08)" : "rgba(220, 38, 38, 0.08)",
        }}
      >
        <span style={{ ...labelStyle, color: ok ? "#059669" : "#dc2626" }}>END {p.attempt}</span>
        <span style={{ color: "var(--text-primary)" }}>
          {uiText(
            `第 ${p.attempt} 轮结束 · 退出码 ${p.return_code} · 耗时 ${sec}s`,
            `Round ${p.attempt} ended · exit ${p.return_code} · ${sec}s`,
            `ラウンド ${p.attempt} 終了 · exit ${p.return_code} · ${sec}s`,
          )}
        </span>
      </div>
    );
  }

  if (event.kind === "stage") {
    return (
      <div style={rowBase}>
        <span style={{ ...labelStyle, color: "var(--accent)" }}>{event.payload.phase}</span>
        <span style={{ color: "var(--text-secondary)" }}>{event.payload.message}</span>
      </div>
    );
  }

  // codex stdout / stderr
  const { kind, raw, json } = event.payload;
  if (kind === "stderr") {
    return (
      <div style={rowBase}>
        <span style={{ ...labelStyle, color: "#dc2626" }}>STDERR</span>
        <span style={{ color: "#dc2626" }}>{raw ?? ""}</span>
      </div>
    );
  }
  if (json !== undefined) {
    const s = summarizeCodexJson(json);
    return (
      <div style={rowBase}>
        <span style={{ ...labelStyle, color: s.tone }}>{s.label}</span>
        <span style={{ color: "var(--text-secondary)" }}>{s.detail}</span>
      </div>
    );
  }
  return (
    <div style={rowBase}>
      <span style={labelStyle}>stdout</span>
      <span style={{ color: "var(--text-secondary)" }}>{raw ?? ""}</span>
    </div>
  );
}
