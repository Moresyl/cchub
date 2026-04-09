import { memo } from "react";
import { Save, X } from "lucide-react";
import type { ReactNode } from "react";

interface ProfileEditorProps {
  title: string;
  subtitle: string;
  onClose: () => void;
  onSave: () => void;
  saveDisabled: boolean;
  saving: boolean;
  children: ReactNode;
}

function ProfileEditorComponent({
  title,
  subtitle,
  onClose,
  onSave,
  saveDisabled,
  saving,
  children,
}: ProfileEditorProps) {
  return (
    <div className="animate-in" style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <div className="page-header">
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <button className="btn btn-ghost btn-icon-sm" onClick={onClose} title="Back">
            <X size={18} />
          </button>
          <div>
            <h2 className="page-title">{title}</h2>
            <p className="page-subtitle">{subtitle}</p>
          </div>
        </div>
      </div>

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 20, paddingBottom: 20 }}>
        {children}
      </div>

      <div className="sticky-footer" style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <button className="btn btn-secondary btn-sm" onClick={onClose}>取消</button>
        <button className="btn btn-primary btn-sm" onClick={onSave} disabled={saveDisabled} style={{ gap: 6 }}>
          {saving ? <div className="spinner" style={{ width: 14, height: 14 }} /> : <Save size={14} />}
          保存
        </button>
      </div>
    </div>
  );
}

export default memo(ProfileEditorComponent);
