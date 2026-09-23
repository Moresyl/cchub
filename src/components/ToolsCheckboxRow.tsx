import { memo } from "react";
import { CheckboxField } from "./ui/checkbox-field";

interface ToolsCheckboxRowProps {
  title: string;
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

function ToolsCheckboxRowComponent({ title, label, checked, onChange }: ToolsCheckboxRowProps) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{title}</span>
      </div>
      <CheckboxField checked={checked} onCheckedChange={onChange} label={label} className="text-[12px]" />
    </div>
  );
}

export default memo(ToolsCheckboxRowComponent);
