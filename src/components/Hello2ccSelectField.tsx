import { memo, type ChangeEvent } from "react";

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
      <select
        className="input"
        value={value}
        onChange={(event: ChangeEvent<HTMLSelectElement>) => onChange(fieldKey, event.target.value)}
        style={{ fontSize: 12 }}
      >
        {options.map((option) => (
          <option key={`${fieldKey}-${option.value || "blank"}`} value={option.value}>{option.label}</option>
        ))}
      </select>
    </label>
  );
}

export default memo(Hello2ccSelectFieldComponent);
