import { memo } from "react";
import { CheckboxField } from "./ui/checkbox-field";

interface ToolsCheckboxOptionProps {
  optionKey: string;
  label: string;
  checked: boolean;
  onToggle: (optionKey: string, checked: boolean) => void;
}

function ToolsCheckboxOptionComponent({ optionKey, label, checked, onToggle }: ToolsCheckboxOptionProps) {
  return (
    <CheckboxField
      checked={checked}
      onCheckedChange={(value) => onToggle(optionKey, value)}
      label={label}
      className="text-[12px]"
    />
  );
}

export default memo(ToolsCheckboxOptionComponent);
