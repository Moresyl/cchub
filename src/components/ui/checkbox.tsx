import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check, Minus } from "lucide-react";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "../../lib/cn";

export const Checkbox = forwardRef<
  ElementRef<typeof CheckboxPrimitive.Root>,
  ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>
>(({ className, ...props }, ref) => (
  <CheckboxPrimitive.Root
    ref={ref}
    data-slot="checkbox"
    className={cn(
      "peer grid size-5 shrink-0 place-items-center rounded-[5px] border border-[var(--border-strong)] bg-[var(--bg-input)] text-primary-foreground outline-none transition-[background-color,border-color,box-shadow,opacity] hover:border-[var(--control-border-hover)] focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]/45 focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--bg-app)] disabled:cursor-default disabled:opacity-50 data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=indeterminate]:border-primary data-[state=indeterminate]:bg-primary",
      className,
    )}
    {...props}
  >
    <CheckboxPrimitive.Indicator className="grid place-items-center">
      {props.checked === "indeterminate" ? (
        <Minus size={13} strokeWidth={2.5} aria-hidden="true" />
      ) : (
        <Check size={13} strokeWidth={2.8} aria-hidden="true" />
      )}
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
));
Checkbox.displayName = CheckboxPrimitive.Root.displayName;
