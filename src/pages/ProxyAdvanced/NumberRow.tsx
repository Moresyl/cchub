import { useEffect, useId, useState } from "react";
import { Input } from "../../components/ui/input";

interface NumberRowProps {
  label: string;
  description: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
}

export default function NumberRow({ label, description, value, onChange, min, max }: NumberRowProps) {
  const id = useId();
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const number = draft.trim() === "" ? NaN : Number(draft);
  const valid = Number.isSafeInteger(number) && number >= (min ?? -Infinity) && number <= (max ?? Infinity);

  function commit() {
    const next = Number.isSafeInteger(number) ? Math.max(min ?? -Infinity, Math.min(max ?? Infinity, number)) : value;
    setDraft(String(next));
    if (next !== value) onChange(next);
  }

  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
      <div style={{ minWidth: 0 }}>
        <label
          htmlFor={id}
          style={{ fontSize: 14, fontWeight: "var(--font-weight-medium)", color: "var(--text-primary)" }}
        >
          {label}
        </label>
        <div id={`${id}-description`} style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>
          {description}
        </div>
      </div>
      <Input
        id={id}
        type="number"
        inputMode="numeric"
        step={1}
        style={{ width: 96, textAlign: "right", flexShrink: 0 }}
        value={draft}
        min={min}
        max={max}
        aria-describedby={`${id}-description`}
        aria-invalid={draft !== "" && !valid}
        onChange={(event) => {
          const text = event.target.value;
          setDraft(text);
          const next = text.trim() === "" ? NaN : Number(text);
          if (Number.isSafeInteger(next) && next >= (min ?? -Infinity) && next <= (max ?? Infinity)) onChange(next);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            setDraft(String(value));
          }
        }}
      />
    </div>
  );
}
