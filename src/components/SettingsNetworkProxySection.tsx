import { memo, type ChangeEventHandler, type KeyboardEventHandler } from "react";
import { Check, Wifi } from "lucide-react";

interface SettingsNetworkProxySectionProps {
  title: string;
  description: string;
  hint: string;
  proxyUrl: string;
  proxySaved: boolean;
  saveLabel: string;
  placeholder: string;
  onProxyChange: ChangeEventHandler<HTMLInputElement>;
  onProxyKeyDown: KeyboardEventHandler<HTMLInputElement>;
  onSave: () => void | Promise<void>;
}

function SettingsNetworkProxySectionComponent({
  title,
  description,
  hint,
  proxyUrl,
  proxySaved,
  saveLabel,
  placeholder,
  onProxyChange,
  onProxyKeyDown,
  onSave,
}: SettingsNetworkProxySectionProps) {
  return (
    <div className="section-card">
      <div className="section-card-title">
        <Wifi size={17} style={{ color: "var(--text-secondary)" }} />
        {title}
      </div>
      <p style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 12 }}>
        {description}
      </p>
      <div style={{ display: "flex", gap: 8 }}>
        <input
          className="input"
          style={{ flex: 1, fontFamily: "'JetBrains Mono', monospace", fontSize: 12 }}
          placeholder={placeholder}
          value={proxyUrl}
          onChange={onProxyChange}
          onKeyDown={onProxyKeyDown}
        />
        <button className="btn btn-primary btn-sm" style={{ gap: 5 }} onClick={onSave}>
          {proxySaved ? <Check size={13} style={{ color: "var(--success)" }} /> : <Check size={13} />}
          {saveLabel}
        </button>
      </div>
      <p style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>
        {hint}
      </p>
    </div>
  );
}

export default memo(SettingsNetworkProxySectionComponent);
