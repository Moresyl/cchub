import { useId, type ReactNode } from "react";
import { cn } from "../../lib/cn";
import { Checkbox } from "./checkbox";

interface CheckboxFieldProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  label: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
  variant?: "inline" | "surface";
  className?: string;
}

export function CheckboxField({
  checked,
  onCheckedChange,
  label,
  description,
  disabled = false,
  variant = "inline",
  className,
}: CheckboxFieldProps) {
  const id = useId();
  const labelId = `${id}-label`;
  const descriptionId = `${id}-description`;
  return (
    <label
      htmlFor={id}
      className={cn(
        "flex min-w-0 cursor-pointer items-start gap-2.5 text-[13px] text-foreground disabled:cursor-not-allowed",
        variant === "surface" &&
          "min-h-14 rounded-md border border-border bg-[var(--bg-elevated)]/55 px-3 py-2.5 transition-colors hover:border-[var(--border-strong)] hover:bg-[var(--bg-card-hover)]",
        disabled && "cursor-not-allowed opacity-55",
        className,
      )}
    >
      <Checkbox
        id={id}
        aria-labelledby={labelId}
        aria-describedby={description ? descriptionId : undefined}
        checked={checked}
        disabled={disabled}
        onCheckedChange={(value) => onCheckedChange(value === true)}
        className="mt-0.5"
      />
      <span className="min-w-0">
        <span id={labelId} className="block font-medium leading-5">
          {label}
        </span>
        {description && (
          <span id={descriptionId} className="mt-0.5 block text-[11.5px] leading-[1.45] text-muted-foreground">
            {description}
          </span>
        )}
      </span>
    </label>
  );
}
