import { memo } from "react";

interface SettingsMigrationOverviewCardProps {
  panel: string;
  label: string;
  value: number;
  tone: "danger" | "warning" | "ready" | "neutral";
  helper: string;
  active: boolean;
  activeLabel: string;
  viewLabel: string;
  onFocus: (panel: string) => void | Promise<void>;
}

function SettingsMigrationOverviewCardComponent({
  panel,
  label,
  value,
  tone,
  helper,
  active,
  activeLabel,
  viewLabel,
  onFocus,
}: SettingsMigrationOverviewCardProps) {
  const palette = tone === "danger"
    ? {
        border: "rgba(239, 68, 68, 0.35)",
        background: "rgba(239, 68, 68, 0.08)",
        valueColor: "var(--error)",
        badgeBg: "rgba(239, 68, 68, 0.14)",
        badgeColor: "var(--error)",
      }
    : tone === "warning"
      ? {
          border: "rgba(245, 158, 11, 0.35)",
          background: "rgba(245, 158, 11, 0.08)",
          valueColor: "var(--warning)",
          badgeBg: "rgba(245, 158, 11, 0.14)",
          badgeColor: "var(--warning)",
        }
      : tone === "ready"
        ? {
            border: "rgba(34, 197, 94, 0.28)",
            background: "rgba(34, 197, 94, 0.07)",
            valueColor: "var(--success)",
            badgeBg: "rgba(34, 197, 94, 0.12)",
            badgeColor: "var(--success)",
          }
        : {
            border: "var(--border-color)",
            background: "var(--bg-input)",
            valueColor: "var(--text-primary)",
            badgeBg: "var(--bg-card)",
            badgeColor: "var(--text-secondary)",
          };

  return (
    <button
      type="button"
      onClick={() => onFocus(panel)}
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: palette.background,
        border: `1px solid ${palette.border}`,
        textAlign: "left",
        cursor: "pointer",
        boxShadow: active ? "0 0 0 1px var(--accent-primary)" : "none",
        transform: active ? "translateY(-1px)" : "none",
        transition: "all 160ms ease",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
        <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.4 }}>{label}</div>
        <span
          style={{
            fontSize: 10,
            padding: "3px 7px",
            borderRadius: 999,
            background: palette.badgeBg,
            color: palette.badgeColor,
            whiteSpace: "nowrap",
          }}
        >
          {active ? activeLabel : viewLabel}
        </span>
      </div>
      <div style={{ fontSize: 22, fontWeight: 700, lineHeight: 1.1, color: palette.valueColor }}>{value}</div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>{helper}</div>
    </button>
  );
}

export default memo(SettingsMigrationOverviewCardComponent);
