import { useEffect } from "react";
import { AlertTriangle, Info } from "lucide-react";

interface ConfirmDialogProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: "destructive" | "info";
  onConfirm: () => void;
  onCancel: () => void;
}

export default function ConfirmDialog({
  isOpen,
  title,
  message,
  confirmText,
  cancelText,
  variant = "destructive",
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, onCancel]);

  if (!isOpen) return null;

  const isDestructive = variant === "destructive";
  const Icon = isDestructive ? AlertTriangle : Info;
  const iconColor = isDestructive ? "var(--danger)" : "var(--accent)";
  const iconBg = isDestructive ? "var(--danger-subtle)" : "var(--accent-subtle)";

  return (
    <div className="confirm-overlay" onClick={onCancel}>
      <div className="confirm-dialog animate-in" onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", gap: 14, alignItems: "flex-start" }}>
          <div style={{
            width: 40, height: 40, borderRadius: 10,
            display: "flex", alignItems: "center", justifyContent: "center",
            background: iconBg, flexShrink: 0,
          }}>
            <Icon size={20} style={{ color: iconColor }} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h3 style={{ fontSize: 15, fontWeight: 600, marginBottom: 6 }}>{title}</h3>
            <p style={{ fontSize: 13, color: "var(--text-muted)", whiteSpace: "pre-line", lineHeight: 1.5 }}>
              {message}
            </p>
          </div>
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
          <button className="btn btn-secondary btn-sm" onClick={onCancel}>
            {cancelText || "取消"}
          </button>
          <button
            className={`btn btn-sm ${isDestructive ? "btn-danger" : "btn-primary"}`}
            onClick={onConfirm}
          >
            {confirmText || "确认"}
          </button>
        </div>
      </div>
    </div>
  );
}
