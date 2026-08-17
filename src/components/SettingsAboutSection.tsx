import { memo } from "react";
import { Info } from "lucide-react";

interface SettingsAboutSectionProps {
  title: string;
  description: string;
  appVersion: string;
  license: string;
  checkUpdateLabel: string;
  onCheckUpdate: () => void;
}

function SettingsAboutSectionComponent({
  title,
  description,
  appVersion,
  license,
  checkUpdateLabel,
  onCheckUpdate,
}: SettingsAboutSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Info size={17} style={{ color: "var(--text-secondary)" }} />
        {title}
      </div>
      <p style={{ fontSize: 13, color: "var(--text-secondary)" }}>{description}</p>
      <div style={{ display: "flex", gap: 12, marginTop: 12, alignItems: "center", flexWrap: "wrap" }}>
        <span className="badge badge-muted">{appVersion}</span>
        <span className="badge badge-muted">{license}</span>
        <button className="btn btn-secondary btn-sm" type="button" onClick={onCheckUpdate}>
          {checkUpdateLabel}
        </button>
      </div>
    </div>
  );
}

export default memo(SettingsAboutSectionComponent);
