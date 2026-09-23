import { memo } from "react";
import { ArrowLeft, Save } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "./ui/button";

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
    <div className="profile-editor animate-in">
      <header className="profile-editor-header">
        <div className="profile-editor-header-inner">
          <Button variant="ghost" size="icon" onClick={onClose} title="返回" aria-label="返回">
            <ArrowLeft size={17} />
          </Button>
          <div>
            <h2>{title}</h2>
            <p>{subtitle}</p>
          </div>
        </div>
      </header>

      <div className="profile-editor-scroll">
        <div className="profile-editor-content">{children}</div>
      </div>

      <footer className="profile-editor-footer">
        <div className="profile-editor-footer-inner">
          <Button variant="secondary" size="sm" onClick={onClose}>
            取消
          </Button>
          <Button size="sm" onClick={onSave} disabled={saveDisabled}>
            {saving ? <div className="spinner profile-editor-save-spinner" /> : <Save size={14} />}
            保存
          </Button>
        </div>
      </footer>
    </div>
  );
}

export default memo(ProfileEditorComponent);
