import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check, Minus } from "lucide-react";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "../../lib/cn";

export const Checkbox = forwardRef<
  ElementRef<typeof CheckboxPrimitive.Root>,
  ComponentPropsWithoutRef<typeof CheckboxPrimitive.Root>
>(({ className, ...props }, ref) => (
  <CheckboxPrimitive.Root ref={ref} data-slot="checkbox" className={cn("peer ui-checkbox", className)} {...props}>
    <CheckboxPrimitive.Indicator className="grid place-items-center">
      <Minus className="ui-checkbox-minus" size={12} strokeWidth={2.5} aria-hidden="true" />
      <Check className="ui-checkbox-check" size={12} strokeWidth={2.5} aria-hidden="true" />
    </CheckboxPrimitive.Indicator>
  </CheckboxPrimitive.Root>
));
Checkbox.displayName = CheckboxPrimitive.Root.displayName;
