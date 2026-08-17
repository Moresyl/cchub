import { PackagePlus, X } from "lucide-react";
import type { Locale } from "../../lib/i18n";

interface PluginInstallDialogProps {
  isOpen: boolean;
  source: string;
  setSource: (value: string) => void;
  busy: boolean;
  locale: Locale;
  onConfirm: () => void;
  onCancel: () => void;
}

export default function PluginInstallDialog({
  isOpen,
  source,
  setSource,
  busy,
  locale,
  onConfirm,
  onCancel,
}: PluginInstallDialogProps) {
  if (!isOpen) return null;

  const zh = locale === "zh";
  return (
    <div className="confirm-overlay" onClick={onCancel}>
      <div
        className="confirm-dialog animate-in"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="plugin-install-title"
      >
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
          <div
            style={{
              width: 40,
              height: 40,
              borderRadius: 10,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: "var(--accent-subtle)",
              color: "var(--accent)",
              flexShrink: 0,
            }}
          >
            <PackagePlus size={20} />
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h3 id="plugin-install-title" style={{ fontSize: 15, fontWeight: 600, marginBottom: 6 }}>
              {zh ? "安装 Claude 插件" : "Install Claude plugin"}
            </h3>
            <p style={{ fontSize: 13, color: "var(--text-muted)", lineHeight: 1.5 }}>
              {zh
                ? "输入 HTTPS 插件归档地址，或选择本机 ZIP/TAR 文件路径。安装会自动备份同名旧插件。"
                : "Enter an HTTPS archive URL or a local ZIP/TAR path. Existing plugins are backed up before replacement."}
            </p>
          </div>
          <button className="btn btn-ghost btn-icon-sm" onClick={onCancel} title={zh ? "关闭" : "Close"}>
            <X size={16} />
          </button>
        </div>
        <input
          className="input"
          style={{ width: "100%", marginTop: 16 }}
          value={source}
          onChange={(event) => setSource(event.target.value)}
          placeholder="https://example.com/plugin.zip"
          aria-label={zh ? "插件归档地址" : "Plugin archive URL"}
          autoFocus
          onKeyDown={(event) => {
            if (event.key === "Enter" && source.trim() && !busy) onConfirm();
          }}
        />
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 20 }}>
          <button className="btn btn-secondary btn-sm" onClick={onCancel} disabled={busy}>
            {zh ? "取消" : "Cancel"}
          </button>
          <button className="btn btn-primary btn-sm" onClick={onConfirm} disabled={!source.trim() || busy}>
            {busy ? (zh ? "安装中..." : "Installing...") : zh ? "安装" : "Install"}
          </button>
        </div>
      </div>
    </div>
  );
}
