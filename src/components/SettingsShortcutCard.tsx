import { memo } from "react";

interface SettingsShortcutCardProps {
  shortcutKey: string;
  description: string;
}

function SettingsShortcutCardComponent({
  shortcutKey,
  description,
}: SettingsShortcutCardProps) {
  return (
    <div style={{ padding: "12px 14px", borderRadius: 10, background: "var(--bg-input)" }}>
      <div style={{ marginBottom: 6 }}>
        <code className="badge badge-accent" style={{ fontFamily: "'JetBrains Mono', monospace" }}>{shortcutKey}</code>
      </div>
      <div style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.6 }}>{description}</div>
    </div>
  );
}

export default memo(SettingsShortcutCardComponent);
