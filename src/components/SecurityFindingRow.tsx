import { memo, type ReactNode } from "react";

export interface SecurityFindingRowFinding {
  category: string;
  severity: string;
  title: string;
  description: string;
}

interface SecurityFindingRowProps {
  finding: SecurityFindingRowFinding;
  categoryLabel: string;
  icon: ReactNode;
}

function SecurityFindingRowComponent({
  finding,
  categoryLabel,
  icon,
}: SecurityFindingRowProps) {
  return (
    <div
      style={{
        display: "flex",
        gap: 10,
        padding: "12px 16px",
        borderRadius: 6,
        background: "var(--bg-elevated)",
      }}
    >
      {icon}
      <div style={{ flex: 1 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{finding.title}</span>
          <span className="badge badge-muted" style={{ fontSize: 10 }}>{categoryLabel}</span>
        </div>
        <p style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5 }}>{finding.description}</p>
      </div>
    </div>
  );
}

export default memo(SecurityFindingRowComponent);
