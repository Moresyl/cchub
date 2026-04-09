import { memo } from "react";
import { Link2 } from "lucide-react";
import SettingsChoiceButton from "./SettingsChoiceButton";
import SettingsToggleRow from "./SettingsToggleRow";

interface SettingsGeneralSectionProps {
  title: string;
  autoScanTitle: string;
  autoScanDescription: string;
  autoScanEnabled: boolean;
  checkUpdatesTitle: string;
  checkUpdatesDescription: string;
  checkUpdatesEnabled: boolean;
  skillSyncTitle: string;
  skillSyncDescription: string;
  skillSyncMethod: "symlink" | "copy";
  skillSyncSymlinkLabel: string;
  skillSyncCopyLabel: string;
  skillSyncSymlinkHint: string;
  onToggleAutoScan: () => void | Promise<void>;
  onToggleCheckUpdates: () => void | Promise<void>;
  onSkillSyncMethodChange: (method: "symlink" | "copy") => void | Promise<void>;
}

function SettingsGeneralSectionComponent({
  title,
  autoScanTitle,
  autoScanDescription,
  autoScanEnabled,
  checkUpdatesTitle,
  checkUpdatesDescription,
  checkUpdatesEnabled,
  skillSyncTitle,
  skillSyncDescription,
  skillSyncMethod,
  skillSyncSymlinkLabel,
  skillSyncCopyLabel,
  skillSyncSymlinkHint,
  onToggleAutoScan,
  onToggleCheckUpdates,
  onSkillSyncMethodChange,
}: SettingsGeneralSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">{title}</div>

      <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
        <SettingsToggleRow
          title={autoScanTitle}
          description={autoScanDescription}
          enabled={autoScanEnabled}
          onToggle={onToggleAutoScan}
        />

        <div className="divider" />

        <SettingsToggleRow
          title={checkUpdatesTitle}
          description={checkUpdatesDescription}
          enabled={checkUpdatesEnabled}
          onToggle={onToggleCheckUpdates}
        />

        <div className="divider" />

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
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: -8 }}>
            {skillSyncSymlinkHint}
          </p>
        )}
      </div>
    </div>
  );
}

export default memo(SettingsGeneralSectionComponent);
