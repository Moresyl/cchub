import { memo } from "react";

export interface DashboardServerRowServer {
  id: string;
  name: string;
  command: string | null;
  args: string;
  env: string;
  status: string;
  transport: string;
  source: string;
}

interface DashboardServerRowProps {
  server: DashboardServerRowServer;
  officialLabel: string;
  communityLabel: string;
  localLabel: string;
}

function DashboardServerRowComponent({
  server,
  officialLabel,
  communityLabel,
  localLabel,
}: DashboardServerRowProps) {
  return (
    <div className="list-row" style={{ padding: "10px 12px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span className={`dot ${server.status === "active" ? "dot-active" : server.status === "error" ? "dot-error" : "dot-disabled"}`} />
        <span style={{ fontSize: 13, fontWeight: 500 }}>{server.name}</span>
      </div>
      <span className={`badge ${server.source === "official-plugin" ? "badge-accent" : server.source === "community-plugin" ? "badge-success" : "badge-muted"}`} style={{ fontSize: 10 }}>
        {server.source === "official-plugin" ? officialLabel : server.source === "community-plugin" ? communityLabel : server.source === "cursor" ? "Cursor" : localLabel}
      </span>
    </div>
  );
}

export default memo(DashboardServerRowComponent);
