import { memo } from "react";

interface SettingsSummaryStatRowProps {
  label: string;
  value: string | number;
}

function SettingsSummaryStatRowComponent({ label, value }: SettingsSummaryStatRowProps) {
  return (
    <div style={{ fontSize: 12 }}>
      <span style={{ color: "var(--text-muted)" }}>{label}: </span>
      <span style={{ fontFamily: "var(--font-code)" }}>{value}</span>
    </div>
  );
}

export default memo(SettingsSummaryStatRowComponent);
