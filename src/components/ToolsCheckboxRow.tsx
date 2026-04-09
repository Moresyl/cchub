import { memo, type ChangeEvent } from "react";

interface ToolsCheckboxRowProps {
  title: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

function ToolsCheckboxRowComponent({
  title,
  label,
  checked,
  onChange,
}: ToolsCheckboxRowProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{title}</span>
      </div>
      <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, cursor: "pointer" }}>
        <input
          type="checkbox"
          checked={checked}
          onChange={(event: ChangeEvent<HTMLInputElement>) => onChange(event.target.checked)}
        />
        {label}
      </label>
    </div>
  );
}

export default memo(ToolsCheckboxRowComponent);
