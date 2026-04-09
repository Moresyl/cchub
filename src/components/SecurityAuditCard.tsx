import { memo, type ReactNode } from "react";
import { ChevronDown, ChevronRight, Shield } from "lucide-react";
import SecurityFindingRow, { type SecurityFindingRowFinding } from "./SecurityFindingRow";

export interface SecurityAuditCardResult {
  server_id: string;
  server_name: string;
  risk_level: string;
  findings: SecurityFindingRowFinding[];
  scanned_at: string;
}

interface SecurityAuditCardProps {
  result: SecurityAuditCardResult;
  expanded: boolean;
  findingsCountLabel: string;
  noIssuesLabel: string;
  riskBadge: ReactNode;
  onToggle: (id: string) => void;
  getFindingIcon: (severity: string) => ReactNode;
  getCategoryLabel: (category: string) => string;
}

function SecurityAuditCardComponent({
  result,
  expanded,
  findingsCountLabel,
  noIssuesLabel,
  riskBadge,
  onToggle,
  getFindingIcon,
  getCategoryLabel,
}: SecurityAuditCardProps) {
  return (
    <div className="card" style={{ padding: "20px 24px" }}>
      <div
        style={{ display: "flex", alignItems: "center", justifyContent: "space-between", cursor: "pointer" }}
        onClick={() => onToggle(result.server_id)}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div style={{ cursor: "pointer" }}>
            {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </div>
          <Shield
            size={18}
            style={{
              color:
                result.risk_level === "high"
                  ? "var(--danger)"
                  : result.risk_level === "medium"
                    ? "var(--warning)"
                    : "var(--success)",
            }}
          />
          <span style={{ fontSize: 14, fontWeight: 600 }}>{result.server_name}</span>
          {riskBadge}
          {result.findings.length > 0 && (
            <span className="badge badge-muted">{findingsCountLabel}</span>
          )}
        </div>
      </div>

      {expanded && result.findings.length > 0 && (
        <div style={{ marginTop: 16, paddingLeft: 28, display: "flex", flexDirection: "column", gap: 10 }}>
          {result.findings.map((finding, index) => (
            <SecurityFindingRow
              key={`${result.server_id}-${index}-${finding.title}`}
              finding={finding}
              categoryLabel={getCategoryLabel(finding.category)}
              icon={getFindingIcon(finding.severity)}
            />
          ))}
        </div>
      )}

      {expanded && result.findings.length === 0 && (
        <div style={{ marginTop: 12, paddingLeft: 28 }}>
          <p style={{ fontSize: 12, color: "var(--success)" }}>{noIssuesLabel}</p>
        </div>
      )}
    </div>
  );
}

export default memo(SecurityAuditCardComponent);
