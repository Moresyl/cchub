import { memo } from "react";
import { Link2 } from "lucide-react";
import SettingsChoiceButton from "./SettingsChoiceButton";

interface SettingsGeneralSectionProps {
  title: string;
  skillSyncTitle: string;
  skillSyncDescription: string;
  skillSyncMethod: "symlink" | "copy";
  skillSyncSymlinkLabel: string;
  skillSyncCopyLabel: string;
  skillSyncSymlinkHint: string;
  onSkillSyncMethodChange: (method: "symlink" | "copy") => void | Promise<void>;
}

function SettingsGeneralSectionComponent({
  title,
  skillSyncTitle,
  skillSyncDescription,
  skillSyncMethod,
  skillSyncSymlinkLabel,
  skillSyncCopyLabel,
  skillSyncSymlinkHint,
  onSkillSyncMethodChange,
}: SettingsGeneralSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">{title}</div>

      <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <Link2 size={15} style={{ color: "var(--text-secondary)" }} />
              <p style={{ fontSize: 14, fontWeight: 500 }}>{skillSyncTitle}</p>
            </div>
            <p style={{ fontSize: 12, color: "var(--text-muted)", marginTop: 3 }}>{skillSyncDescription}</p>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <SettingsChoiceButton
              value="symlink"
              label={skillSyncSymlinkLabel}
              active={skillSyncMethod === "symlink"}
              onSelect={onSkillSyncMethodChange}
            />
            <SettingsChoiceButton
              value="copy"
              label={skillSyncCopyLabel}
              active={skillSyncMethod === "copy"}
              onSelect={onSkillSyncMethodChange}
            />
          </div>
        </div>
        {skillSyncMethod === "symlink" && (
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: -8 }}>{skillSyncSymlinkHint}</p>
        )}
      </div>
    </div>
  );
}

export default memo(SettingsGeneralSectionComponent);
