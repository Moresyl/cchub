import { type ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./select";

export interface SimpleSelectOption {
  value: string;
  label: ReactNode;
  disabled?: boolean;
}

interface SimpleSelectProps {
  id?: string;
  value: string;
  options: readonly SimpleSelectOption[];
  onValueChange: (value: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  ariaDescribedBy?: string;
  disabled?: boolean;
  className?: string;
  contentClassName?: string;
  controlSize?: "xs" | "sm" | "md" | "lg";
}

const EMPTY_OPTION_VALUE = "__cchub_empty_option__";

export function SimpleSelect({
  id,
  value,
  options,
  onValueChange,
  placeholder,
  ariaLabel,
  ariaDescribedBy,
  disabled,
  className,
  contentClassName,
  controlSize,
}: SimpleSelectProps) {
  const selectValue = value === "" ? EMPTY_OPTION_VALUE : value;
  return (
    <Select
      value={selectValue}
      onValueChange={(nextValue) => onValueChange(nextValue === EMPTY_OPTION_VALUE ? "" : nextValue)}
      disabled={disabled}
    >
      <SelectTrigger
        id={id}
        className={cn(className)}
        aria-label={ariaLabel}
        aria-describedby={ariaDescribedBy}
        controlSize={controlSize}
      >
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent className={contentClassName}>
        {options.map((option) => (
          <SelectItem
            key={option.value || EMPTY_OPTION_VALUE}
            value={option.value || EMPTY_OPTION_VALUE}
            disabled={option.disabled}
          >
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
