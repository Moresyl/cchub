import { memo } from "react";

interface SettingsSummaryMetricCardProps {
  label: string;
  value: string;
}

function SettingsSummaryMetricCardComponent({ label, value }: SettingsSummaryMetricCardProps) {
  return (
    <div style={{ padding: "8px 10px", borderRadius: 8, background: "var(--bg-card)" }}>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 12, fontFamily: "var(--font-code)", wordBreak: "break-word" }}>{value}</div>
    </div>
  );
}

export default memo(SettingsSummaryMetricCardComponent);
