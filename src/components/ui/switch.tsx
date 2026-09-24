import * as SwitchPrimitive from "@radix-ui/react-switch";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "../../lib/cn";

export const Switch = forwardRef<
  ElementRef<typeof SwitchPrimitive.Root>,
  ComponentPropsWithoutRef<typeof SwitchPrimitive.Root>
>(({ className, ...props }, ref) => (
  <SwitchPrimitive.Root
    ref={ref}
    data-slot="switch"
    className={cn(
      "group relative inline-flex h-[19px] w-8 shrink-0 cursor-pointer items-center rounded-full border-0 bg-[var(--switch-track)] outline-none transition-[background-color,box-shadow,opacity] duration-150 hover:bg-[var(--switch-track-hover)] focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]/45 focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--bg-app)] disabled:cursor-default disabled:opacity-50 data-[state=checked]:bg-[var(--switch-track-checked)] data-[state=checked]:hover:bg-[var(--switch-track-checked)]",
      className,
    )}
    {...props}
  >
    <SwitchPrimitive.Thumb className="pointer-events-none block size-[13px] translate-x-[3px] rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.2)] transition-transform duration-150 ease-out data-[state=checked]:translate-x-4" />
  </SwitchPrimitive.Root>
));
Switch.displayName = SwitchPrimitive.Root.displayName;
