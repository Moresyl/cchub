import { memo } from "react";
import { getAppLabel, type ManagedAppId } from "../lib/appPreferences";

interface SettingsManagedAppToggleProps {
  appId: ManagedAppId;
  active: boolean;
  disabled: boolean;
  onToggle: (appId: ManagedAppId) => void | Promise<void>;
}

function SettingsManagedAppToggleComponent({
  appId,
  active,
  disabled,
  onToggle,
}: SettingsManagedAppToggleProps) {
  return (
    <button
      className={`btn btn-sm ${active ? "btn-primary" : "btn-secondary"}`}
      onClick={() => onToggle(appId)}
      disabled={disabled}
      style={{ gap: 6 }}
    >
      {getAppLabel(appId)}
      <span className={`dot ${active ? "dot-active" : "dot-disabled"}`} />
    </button>
  );
}

export default memo(SettingsManagedAppToggleComponent);
