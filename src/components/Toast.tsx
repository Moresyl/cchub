import { memo, useState, useEffect, useCallback } from "react";
import { CheckCircle, AlertCircle, Info, X } from "lucide-react";
import { t } from "../lib/i18n";
import { Button } from "./ui/button";

export type ToastType = "success" | "error" | "info";

interface Toast {
  id: string;
  type: ToastType;
  message: string;
}

interface ToastItemProps {
  toast: Toast;
  onDismiss: (id: string) => void;
}

let addToastFn: ((type: ToastType, message: string, duration?: number) => void) | null = null;

export function showToast(type: ToastType, message: string, duration?: number) {
  addToastFn?.(type, message, duration);
}

function ToastItemComponent({ toast, onDismiss }: ToastItemProps) {
  const icons = { success: CheckCircle, error: AlertCircle, info: Info };
  const colors = {
    success: { bg: "var(--success-subtle)", icon: "var(--success)" },
    error: { bg: "var(--danger-subtle)", icon: "var(--danger)" },
    info: { bg: "var(--accent-subtle)", icon: "var(--accent)" },
  };
  const Icon = icons[toast.type];
  const color = colors[toast.type];
  const handleDismiss = useCallback(() => {
    onDismiss(toast.id);
  }, [onDismiss, toast.id]);

  return (
    <div
      role={toast.type === "error" ? "alert" : "status"}
      className="app-toast"
      style={{ borderLeftColor: color.icon }}
    >
      <span className="app-toast-icon" style={{ color: color.icon, background: color.bg }}>
        <Icon size={16} aria-hidden="true" />
      </span>
      <span className="app-toast-message">{toast.message}</span>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="app-toast-close"
        aria-label={t().common.close}
        title={t().common.close}
        onClick={handleDismiss}
      >
        <X size={14} aria-hidden="true" />
      </Button>
    </div>
  );
}

const ToastItem = memo(ToastItemComponent);

export const ToastContainer = memo(function ToastContainer() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const addToast = useCallback((type: ToastType, message: string, duration?: number) => {
    const id = Date.now().toString() + Math.random().toString(36).slice(2);
    // 限制最多 5 条同时显示，避免出现错误风暴时大量 toast 同时入栈占满屏幕
    // 并阻塞用户操作（每条 toast 都要计算 layout、消失时还会触发 N 次 setState）。
    setToasts((prev) => {
      const next = [...prev, { id, type, message }];
      return next.length > 5 ? next.slice(next.length - 5) : next;
    });
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, duration ?? 4000);
  }, []);

  useEffect(() => {
    addToastFn = addToast;
    return () => {
      addToastFn = null;
    };
  }, [addToast]);

  if (toasts.length === 0) return null;

  return (
    <div className="app-toast-stack" aria-live="polite" aria-relevant="additions">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={dismissToast} />
      ))}
    </div>
  );
});
