import { memo, useState, useEffect, useCallback } from "react";
import { CheckCircle, AlertCircle, Info, X } from "lucide-react";

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

let addToastFn: ((type: ToastType, message: string) => void) | null = null;

export function showToast(type: ToastType, message: string) {
  addToastFn?.(type, message);
}

function ToastItemComponent({ toast, onDismiss }: ToastItemProps) {
  const icons = { success: CheckCircle, error: AlertCircle, info: Info };
  const colors = {
    success: { bg: "var(--success-subtle)", border: "var(--success)", icon: "var(--success)" },
    error: { bg: "var(--danger-subtle)", border: "var(--danger)", icon: "var(--danger)" },
    info: { bg: "var(--accent-subtle)", border: "var(--accent)", icon: "var(--accent)" },
  };
  const Icon = icons[toast.type];
  const color = colors[toast.type];
  const handleDismiss = useCallback(() => {
    onDismiss(toast.id);
  }, [onDismiss, toast.id]);

  return (
    <div
      style={{
        display: "flex", alignItems: "flex-start", gap: 10, padding: "12px 16px",
        borderRadius: 8, background: color.bg, border: `1px solid ${color.border}`,
        animation: "slideIn 0.2s ease",
      }}
    >
      <Icon size={16} style={{ color: color.icon, flexShrink: 0, marginTop: 1 }} />
      <span style={{ fontSize: 13, color: "var(--text-primary)", flex: 1, lineHeight: 1.5 }}>{toast.message}</span>
      <button
        onClick={handleDismiss}
        style={{ background: "none", border: "none", cursor: "pointer", padding: 0, color: "var(--text-muted)" }}
      >
        <X size={14} />
      </button>
    </div>
  );
}

const ToastItem = memo(ToastItemComponent);

export const ToastContainer = memo(function ToastContainer() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  const dismissToast = useCallback((id: string) => {
    setToasts(prev => prev.filter(t => t.id !== id));
  }, []);

  const addToast = useCallback((type: ToastType, message: string) => {
    const id = Date.now().toString() + Math.random().toString(36).slice(2);
    setToasts(prev => [...prev, { id, type, message }]);
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 4000);
  }, []);

  useEffect(() => {
    addToastFn = addToast;
    return () => { addToastFn = null; };
  }, [addToast]);

  if (toasts.length === 0) return null;

  return (
    <div style={{ position: "fixed", top: 12, left: "50%", transform: "translateX(-50%)", zIndex: 9999, display: "flex", flexDirection: "column", gap: 8, maxWidth: 480, minWidth: 300 }}>
      {toasts.map(toast => (
        <ToastItem
          key={toast.id}
          toast={toast}
          onDismiss={dismissToast}
        />
      ))}
    </div>
  );
});
