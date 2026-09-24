import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import { cn } from "../../lib/cn";

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export const DialogOverlay = forwardRef<
  ElementRef<typeof DialogPrimitive.Overlay>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    ref={ref}
    className={cn(
      "fixed inset-0 z-[2000] bg-[color-mix(in_srgb,var(--bg-app)_74%,transparent)] backdrop-blur-[3px] data-[state=closed]:animate-[dialog-fade-out_120ms_ease-in] data-[state=open]:animate-[dialog-fade-in_140ms_ease-out]",
      className,
    )}
    {...props}
  />
));
DialogOverlay.displayName = DialogPrimitive.Overlay.displayName;

export const DialogContent = forwardRef<
  ElementRef<typeof DialogPrimitive.Content>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Content> & { hideClose?: boolean }
>(({ className, children, hideClose = false, ...props }, ref) => (
  <DialogPrimitive.Portal>
    <DialogOverlay />
    <DialogPrimitive.Content
      ref={ref}
      data-slot="dialog-content"
      className={cn(
        "fixed left-1/2 top-[48%] z-[2001] flex max-h-[min(460px,calc(100dvh-40px))] w-[calc(100vw-32px)] max-w-[450px] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-[var(--border-strong)] bg-card text-card-foreground shadow-[var(--shadow-lg)] outline-none data-[state=closed]:animate-[dialog-out_120ms_ease-in] data-[state=open]:animate-[dialog-in_160ms_cubic-bezier(0.22,1,0.36,1)] max-md:inset-0 max-md:max-h-none max-md:w-full max-md:max-w-none max-md:translate-x-0 max-md:translate-y-0 max-md:rounded-none",
        className,
      )}
      {...props}
    >
      {children}
      {!hideClose && (
        <DialogPrimitive.Close className="absolute right-4 top-4 grid size-7 place-items-center rounded-[5px] text-muted-foreground transition-colors hover:bg-[var(--bg-card-hover)] hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--control-border-focus)]">
          <X size={14} aria-hidden="true" />
          <span className="sr-only">Close</span>
        </DialogPrimitive.Close>
      )}
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
));
DialogContent.displayName = DialogPrimitive.Content.displayName;

export const DialogHeader = ({ className, ...props }: ComponentPropsWithoutRef<"div">) => (
  <div className={cn("flex shrink-0 items-start gap-3 border-b border-border px-5 py-4 pr-14", className)} {...props} />
);

export const DialogTitle = forwardRef<
  ElementRef<typeof DialogPrimitive.Title>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title ref={ref} className={cn("text-[14px] font-[590]", className)} {...props} />
));
DialogTitle.displayName = DialogPrimitive.Title.displayName;

export const DialogDescription = forwardRef<
  ElementRef<typeof DialogPrimitive.Description>,
  ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn("mt-1 text-[12px] leading-relaxed text-muted-foreground", className)}
    {...props}
  />
));
DialogDescription.displayName = DialogPrimitive.Description.displayName;

export const DialogBody = ({ className, ...props }: ComponentPropsWithoutRef<"div">) => (
  <div className={cn("min-h-0 flex-1 overflow-y-auto p-5", className)} {...props} />
);

export const DialogFooter = ({ className, ...props }: ComponentPropsWithoutRef<"div">) => (
  <div
    className={cn(
      "flex shrink-0 items-center justify-end gap-2 border-t border-border bg-[var(--bg-elevated)]/55 px-5 py-3.5",
      className,
    )}
    {...props}
  />
);
