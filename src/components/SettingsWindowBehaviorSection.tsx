import { memo } from "react";
import { Info } from "lucide-react";
import SettingsToggleRow from "./SettingsToggleRow";

interface SettingsWindowBehaviorSectionProps {
  title: string;
  launchAtLoginTitle: string;
  launchAtLoginDescription: string;
  launchAtLoginEnabled: boolean;
  launchHiddenTitle: string;
  launchHiddenDescription: string;
  launchHiddenEnabled: boolean;
  closeToTrayTitle: string;
  closeToTrayDescription: string;
  closeToTrayEnabled: boolean;
  savingWindowKey: "launch_at_login" | "launch_hidden" | "close_to_tray" | null;
  onToggleLaunchAtLogin: () => void | Promise<void>;
  onToggleLaunchHidden: () => void | Promise<void>;
  onToggleCloseToTray: () => void | Promise<void>;
}

function SettingsWindowBehaviorSectionComponent({
  title,
  launchAtLoginTitle,
  launchAtLoginDescription,
  launchAtLoginEnabled,
  launchHiddenTitle,
  launchHiddenDescription,
  launchHiddenEnabled,
  closeToTrayTitle,
  closeToTrayDescription,
  closeToTrayEnabled,
  savingWindowKey,
  onToggleLaunchAtLogin,
  onToggleLaunchHidden,
  onToggleCloseToTray,
}: SettingsWindowBehaviorSectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Info size={17} style={{ color: "var(--text-secondary)" }} />
        {title}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 18 }}>
        <SettingsToggleRow
          title={launchAtLoginTitle}
          description={launchAtLoginDescription}
          enabled={launchAtLoginEnabled}
          disabled={savingWindowKey === "launch_at_login"}
          onToggle={onToggleLaunchAtLogin}
        />

        <div className="divider" />

        <SettingsToggleRow
          title={launchHiddenTitle}
          description={launchHiddenDescription}
          enabled={launchHiddenEnabled}
          disabled={savingWindowKey === "launch_hidden"}
          onToggle={onToggleLaunchHidden}
        />

        <div className="divider" />

        <SettingsToggleRow
          title={closeToTrayTitle}
          description={closeToTrayDescription}
          enabled={closeToTrayEnabled}
          disabled={savingWindowKey === "close_to_tray"}
          onToggle={onToggleCloseToTray}
        />
      </div>
    </div>
  );
}

export default memo(SettingsWindowBehaviorSectionComponent);
