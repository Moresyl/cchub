import { memo } from "react";
import { AlertTriangle, Info } from "lucide-react";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";

interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: "destructive" | "info";
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

function ConfirmDialogComponent({
  isOpen,
  title,
  message,
  confirmText,
  cancelText,
  variant = "destructive",
  busy = false,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const isDestructive = variant === "destructive";
  const Icon = isDestructive ? AlertTriangle : Info;
  const iconColor = isDestructive ? "var(--danger)" : "var(--accent)";
  const iconBg = isDestructive ? "var(--danger-subtle)" : "var(--accent-subtle)";

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && !busy && onCancel()}>
      <DialogContent hideClose fullscreenOnMobile={false} className="max-w-[420px]">
        <DialogHeader className="min-h-0 shrink overflow-y-auto pr-5">
          <div
            className="grid size-9 shrink-0 place-items-center rounded-[7px]"
            style={{ background: iconBg, color: iconColor }}
          >
            <Icon size={17} aria-hidden="true" />
          </div>
          <div className="min-w-0">
            <DialogTitle className="break-words">{title}</DialogTitle>
            <DialogDescription className="whitespace-pre-line break-words">{message}</DialogDescription>
          </div>
        </DialogHeader>
        <DialogFooter className="flex-wrap">
          <Button variant="secondary" disabled={busy} onClick={onCancel}>
            {cancelText || "取消"}
          </Button>
          <Button variant={isDestructive ? "destructive" : "default"} disabled={busy} onClick={onConfirm}>
            {confirmText || "确认"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export default memo(ConfirmDialogComponent);
