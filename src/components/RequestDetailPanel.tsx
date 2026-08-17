import { memo } from "react";

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
}

interface Props {
  record: RequestDetailRecord | null;
  loading: boolean;
  title: string;
  closeLabel: string;
  onClose: () => void;
}

function RequestDetailPanel({ record, loading, title, closeLabel, onClose }: Props) {
  if (!record && !loading) return null;
  return (
    <div className="card" style={{ marginTop: 12, padding: 12, borderColor: "var(--border-strong)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{title}</span>
        <button className="btn btn-ghost btn-xs" type="button" onClick={onClose}>
          {closeLabel}
        </button>
      </div>
      {loading ? (
        <div className="spinner" style={{ width: 14, height: 14 }} />
      ) : record ? (
        <div
          style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit,minmax(130px,1fr))", gap: 6, fontSize: 11 }}
        >
          <Detail label="Request" value={record.request_id} />
          <Detail label="Provider" value={record.provider_name} />
          <Detail label="Profile" value={record.profile_id} />
          <Detail label="Status" value={String(record.status_code)} />
          <Detail label="Latency" value={`${record.latency_ms}ms`} />
          <Detail label="Cost" value={record.total_cost_usd} />
          <Detail label="Input / Output" value={`${record.input_tokens} / ${record.output_tokens}`} />
          <Detail label="Mode" value={record.is_streaming ? "streaming" : "standard"} />
          {record.error_message && <Detail label="Error" value={record.error_message} />}
        </div>
      ) : null}
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div style={{ color: "var(--text-muted)" }}>{label}</div>
      <div style={{ wordBreak: "break-word" }}>{value}</div>
    </div>
  );
}

export default memo(RequestDetailPanel);
