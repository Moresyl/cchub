import { forwardRef, type ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/cn";

export const Textarea = forwardRef<HTMLTextAreaElement, ComponentPropsWithoutRef<"textarea">>(
  ({ className, ...props }, ref) => (
    <textarea ref={ref} data-slot="textarea" className={cn("input min-h-24 resize-y", className)} {...props} />
  ),
);
Textarea.displayName = "Textarea";
