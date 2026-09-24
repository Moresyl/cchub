import { memo } from "react";
import { SimpleSelect } from "./ui/simple-select";

export interface Hello2ccSelectOption {
  value: string;
  label: string;
}

interface Hello2ccSelectFieldProps {
  fieldKey: string;
  label: string;
  description: string;
  value: string;
  onChange: (fieldKey: string, value: string) => void;
  options: Hello2ccSelectOption[];
}

function Hello2ccSelectFieldComponent({
  fieldKey,
  label,
  description,
  value,
  onChange,
  options,
}: Hello2ccSelectFieldProps) {
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <span style={{ fontSize: 12, fontWeight: 600 }}>{label}</span>
      <span style={{ fontSize: 11, color: "var(--text-muted)", minHeight: 32 }}>{description}</span>
      <SimpleSelect
        value={value}
        onValueChange={(nextValue) => onChange(fieldKey, nextValue)}
        options={options}
        ariaLabel={label}
      />
    </label>
  );
}

export default memo(Hello2ccSelectFieldComponent);
