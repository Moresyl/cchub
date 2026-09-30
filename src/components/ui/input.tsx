import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  controlSize?: "xs" | "sm" | "md" | "lg";
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, controlSize = "md", ...props }, ref) => (
    <input
      ref={ref}
      type={type}
      data-slot="input"
      data-control-size={controlSize}
      className={cn("input", className)}
      {...props}
    />
  ),
);
Input.displayName = "Input";
