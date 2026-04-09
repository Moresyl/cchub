import { memo } from "react";
import type { LucideIcon } from "lucide-react";
import ToolsActionButton from "./ToolsActionButton";
import ToolsToggleSwitch from "./ToolsToggleSwitch";

interface ManagedAction {
  label: string;
  icon: LucideIcon;
  pending: boolean;
  onClick: () => void;
  disabled?: boolean;
  variant?: "primary" | "secondary";
  title?: string;
  gap?: number;
}

interface ManagedToggle {
  value: boolean;
  onChange: (value: boolean) => void;
  labelOn: string;
  labelOff: string;
}

interface ToolsManagedSectionHeaderProps {
  title: string;
  description: string;
  version?: string;
  installed: boolean;
  installAction: ManagedAction;
  primaryAction?: ManagedAction;
  secondaryAction?: ManagedAction;
  toggle?: ManagedToggle;
  actionsWrap?: boolean;
  marginBottomWhenInstalled?: number;
}

function renderAction(action: ManagedAction) {
  return (
    <ToolsActionButton
      key={action.label}
      label={action.label}
      icon={action.icon}
      pending={action.pending}
      onClick={action.onClick}
      disabled={action.disabled}
      variant={action.variant}
      title={action.title}
      gap={action.gap}
    />
  );
}

function ToolsManagedSectionHeaderComponent({
  title,
  description,
  version,
  installed,
  installAction,
  primaryAction,
  secondaryAction,
  toggle,
  actionsWrap = false,
  marginBottomWhenInstalled = 12,
}: ToolsManagedSectionHeaderProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: installed ? marginBottomWhenInstalled : 0 }}>
      <div>
        <h4 style={{ fontSize: 13, fontWeight: 700 }}>{title}</h4>
        <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
          {description}
          {installed && version ? (
            <span style={{ marginLeft: 6, fontSize: 10, opacity: 0.7 }}>v{version}</span>
          ) : null}
        </p>
      </div>
      {!installed ? (
        renderAction(installAction)
      ) : (
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: actionsWrap ? "wrap" : "nowrap" }}>
          {primaryAction ? renderAction(primaryAction) : null}
          {secondaryAction ? renderAction(secondaryAction) : null}
          {toggle ? (
            <ToolsToggleSwitch
              value={toggle.value}
              onChange={toggle.onChange}
              labelOn={toggle.labelOn}
              labelOff={toggle.labelOff}
            />
          ) : null}
        </div>
      )}
    </div>
  );
}

export default memo(ToolsManagedSectionHeaderComponent);
