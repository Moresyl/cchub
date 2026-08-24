import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-[5px] text-[12px] font-semibold transition-colors duration-100 outline-none focus-visible:ring-2 focus-visible:ring-primary/55 focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-45",
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
        default: "h-9 px-3.5",
        sm: "h-8 px-3 text-[11.5px]",
        lg: "h-10 px-5 text-[13px]",
        icon: "size-8 p-0",
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
