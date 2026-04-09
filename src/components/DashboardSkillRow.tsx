import { memo } from "react";

export interface DashboardSkillRowSkill {
  id: string;
  name: string;
  plugin_id: string | null;
  trigger_command: string | null;
  description: string | null;
}

interface DashboardSkillRowProps {
  skill: DashboardSkillRowSkill;
}

function DashboardSkillRowComponent({
  skill,
}: DashboardSkillRowProps) {
  return (
    <div className="list-row" style={{ padding: "10px 12px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0, flex: 1 }}>
        <span style={{ fontSize: 13, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{skill.name}</span>
        {skill.plugin_id && <span className="badge badge-muted" style={{ fontSize: 10, flexShrink: 0 }}>{skill.plugin_id}</span>}
      </div>
      {skill.trigger_command && (
        <code className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: 10, flexShrink: 0 }}>{skill.trigger_command}</code>
      )}
    </div>
  );
}

export default memo(DashboardSkillRowComponent);
