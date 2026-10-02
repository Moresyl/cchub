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
    <div className="grid grid-cols-[minmax(0,1fr)_96px] items-start gap-x-4 gap-y-1">
      <label htmlFor={id} className="min-w-0 self-center text-sm font-medium text-[var(--text-primary)]">
        {label}
      </label>
      <Input
        id={id}
        type="number"
        inputMode="numeric"
        step={1}
        className="row-span-2 w-24 self-center text-right max-[480px]:row-span-1"
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
      <div id={`${id}-description`} className="col-start-1 text-xs text-[var(--text-secondary)] max-[480px]:col-span-2">
        {description}
      </div>
    </div>
  );
}
