import { memo } from "react";
import Hello2ccSelectField, { type Hello2ccSelectOption } from "./Hello2ccSelectField";
import ToolsToggleSwitch from "./ToolsToggleSwitch";

export interface Hello2ccConfigField {
  fieldKey: string;
  label: string;
  description: string;
  value: string;
  options: Hello2ccSelectOption[];
}

interface Hello2ccConfigSectionProps {
  pathLabel: string;
  installPath: string;
  fields: Hello2ccConfigField[];
  onSelectChange: (fieldKey: string, value: string) => void;
  mirrorTitle: string;
  mirrorDescription: string;
  mirrorValue: boolean;
  onMirrorChange: (value: boolean) => void;
  mirrorLabelOn: string;
  mirrorLabelOff: string;
  resetLabel: string;
  saveLabel: string;
  hasChanges: boolean;
  isSaving: boolean;
  onReset: () => void;
  onSave: () => void;
}

function Hello2ccConfigSectionComponent({
  pathLabel,
  installPath,
  fields,
  onSelectChange,
  mirrorTitle,
  mirrorDescription,
  mirrorValue,
  onMirrorChange,
  mirrorLabelOn,
  mirrorLabelOff,
  resetLabel,
  saveLabel,
  hasChanges,
  isSaving,
  onReset,
  onSave,
}: Hello2ccConfigSectionProps) {
  return (
    <div style={{ borderTop: "1px solid var(--border)", paddingTop: 12, display: "flex", flexDirection: "column", gap: 14 }}>
      <div style={{ fontSize: 11, color: "var(--text-muted)", lineHeight: 1.5 }}>
        <div>{pathLabel}</div>
        <div style={{ marginTop: 4, fontFamily: "var(--font-mono, monospace)", wordBreak: "break-all" }}>{installPath}</div>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 12 }}>
        {fields.map((field) => (
          <Hello2ccSelectField
            key={field.fieldKey}
            fieldKey={field.fieldKey}
            label={field.label}
            description={field.description}
            value={field.value}
            onChange={onSelectChange}
            options={field.options}
          />
        ))}
      </div>

      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <div>
          <h5 style={{ fontSize: 12, fontWeight: 600 }}>{mirrorTitle}</h5>
          <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>{mirrorDescription}</p>
        </div>
        <ToolsToggleSwitch
          value={mirrorValue}
          onChange={onMirrorChange}
          labelOn={mirrorLabelOn}
          labelOff={mirrorLabelOff}
        />
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <button
          className="btn btn-secondary btn-xs"
          onClick={onReset}
          disabled={!hasChanges || isSaving}
        >
          {resetLabel}
        </button>
        <button
          className="btn btn-primary btn-xs"
          onClick={onSave}
          disabled={!hasChanges || isSaving}
          style={{ gap: 5 }}
        >
          {isSaving ? <div className="spinner" style={{ width: 12, height: 12 }} /> : null}
          {saveLabel}
        </button>
      </div>
    </div>
  );
}

export default memo(Hello2ccConfigSectionComponent);
