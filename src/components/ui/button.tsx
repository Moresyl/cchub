import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-[var(--control-radius)] text-[12px] font-normal transition-colors duration-150 outline-none focus-visible:ring-2 focus-visible:ring-[var(--focus-ring)]/40 focus-visible:ring-offset-[-1px] disabled:pointer-events-none disabled:opacity-25",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive: "bg-[var(--danger)] text-white hover:bg-[color-mix(in_srgb,var(--danger)_88%,black)]",
        outline: "border border-border bg-card text-foreground hover:bg-[var(--bg-card-hover)]",
        secondary: "border border-border bg-secondary text-secondary-foreground hover:bg-[var(--bg-card-hover)]",
        ghost: "text-muted-foreground hover:bg-[var(--bg-card-hover)] hover:text-foreground",
      },
      size: {
        default: "h-[var(--control-height-lg)] px-3.5",
        sm: "h-[var(--control-height-md)] px-3 text-[12px]",
        lg: "h-10 px-5 text-[14px]",
        icon: "size-[var(--control-height-md)] p-0",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Component = asChild ? Slot : "button";
    return <Component ref={ref} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
  },
);
Button.displayName = "Button";

export { buttonVariants };
