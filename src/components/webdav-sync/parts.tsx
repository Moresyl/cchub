import { memo, useCallback, type ChangeEvent } from "react";
import { Copy, type LucideIcon } from "lucide-react";

export interface WebDavSyncSettings {
  enabled: boolean;
  base_url: string;
  username: string;
  password: string;
  has_password: boolean;
  remote_root: string;
  profile: string;
  auto_sync: boolean;
  last_sync_at: string | null;
  last_error: string | null;
}

export interface WebDavRemoteInfo {
  exists: boolean;
  remote_url: string;
  snapshot_path: string | null;
  updated_at: string | null;
  size_bytes: number | null;
  app_version: string | null;
  device_name: string | null;
  layout: string | null;
  compatible: boolean;
  protocol_version: number | null;
  db_compat_version: number | null;
  profile_path: string | null;
}

export interface WebDavSyncEvent {
  status: string;
  message: string;
  synced_at: string | null;
  error: string | null;
}

export type ActionState = "idle" | "loading" | "saving" | "testing" | "refreshing" | "uploading" | "downloading";

export interface WebDavPreset {
  id: string;
  label: string;
  baseUrl: string;
  hint: string;
  matchPattern?: string;
}

export type WebDavTextFieldKey = "base_url" | "username" | "password" | "remote_root" | "profile";

export interface WebDavToggleCardProps {
  title: string;
  description: string;
  enabled: boolean;
  disabled: boolean;
  onToggle: () => void;
}

export interface WebDavTextFieldProps {
  fieldKey: WebDavTextFieldKey;
  label: string;
  value: string;
  disabled: boolean;
  onValueChange: (fieldKey: WebDavTextFieldKey, value: string) => void;
  placeholder?: string;
  type?: "text" | "password";
}

export interface WebDavActionButtonProps {
  label: string;
  loading: boolean;
  disabled: boolean;
  icon: LucideIcon;
  variant?: "btn-primary" | "btn-secondary" | "btn-ghost";
  onClick: () => void;
}

export interface WebDavInfoCardProps {
  title: string;
  value: string;
  detail: string;
  valueLarge?: boolean;
  mono?: boolean;
  icon?: LucideIcon;
  iconColor?: string;
}

export interface WebDavSnapshotDetailsProps {
  title: string;
  snapshotLabel: string;
  snapshotPath: string;
  deviceLabel: string;
  deviceName: string;
  manifestLabel: string;
  remoteUrl: string;
  copyTitle: string;
  canCopy: boolean;
  onCopy: () => void;
}

export const EMPTY_SETTINGS: WebDavSyncSettings = {
  enabled: false,
  base_url: "",
  username: "",
  password: "",
  has_password: false,
  remote_root: "cchub-sync",
  profile: "default",
  auto_sync: false,
  last_sync_at: null,
  last_error: null,
};

export const WEBDAV_PRESETS: WebDavPreset[] = [
  {
    id: "jianguoyun",
    label: "坚果云",
    baseUrl: "https://dav.jianguoyun.com/dav/",
    hint: "适合直接填入坚果云账号与应用密码",
    matchPattern: "jianguoyun.com",
  },
  {
    id: "nextcloud",
    label: "Nextcloud",
    baseUrl: "https://your-server/remote.php/dav/files/USERNAME/",
    hint: "多数自建 Nextcloud / OwnCloud 使用这个路径格式",
    matchPattern: "remote.php/dav",
  },
  {
    id: "synology",
    label: "Synology",
    baseUrl: "https://your-nas:5006/",
    hint: "群晖 DSM WebDAV 常见端口为 5005/5006",
    matchPattern: ":5006",
  },
  {
    id: "custom",
    label: "自定义",
    baseUrl: "",
    hint: "任意兼容 WebDAV 的服务端地址",
  },
];

function WebDavToggleCardComponent({ title, description, enabled, disabled, onToggle }: WebDavToggleCardProps) {
  return (
    <div
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-input)",
      }}
    >
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>{title}</div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 12,
          alignItems: "center",
        }}
      >
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>{description}</div>
        <button className={`toggle ${enabled ? "on" : "off"}`} onClick={onToggle} disabled={disabled}>
          <div className="toggle-knob" />
        </button>
      </div>
    </div>
  );
}

export const WebDavToggleCard = memo(WebDavToggleCardComponent);

function WebDavTextFieldComponent({
  fieldKey,
  label,
  value,
  disabled,
  onValueChange,
  placeholder,
  type = "text",
}: WebDavTextFieldProps) {
  const handleChange = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      onValueChange(fieldKey, event.target.value);
    },
    [fieldKey, onValueChange],
  );

  return (
    <div>
      <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 6 }}>{label}</div>
      <input
        className="input"
        value={value}
        placeholder={placeholder}
        onChange={handleChange}
        disabled={disabled}
        type={type}
      />
    </div>
  );
}

export const WebDavTextField = memo(WebDavTextFieldComponent);

function WebDavActionButtonComponent({
  label,
  loading,
  disabled,
  icon: Icon,
  variant = "btn-secondary",
  onClick,
}: WebDavActionButtonProps) {
  return (
    <button className={`btn ${variant} btn-sm`} onClick={onClick} disabled={disabled} style={{ gap: 6 }}>
      <Icon size={14} className={loading ? "spin" : ""} />
      {label}
    </button>
  );
}

export const WebDavActionButton = memo(WebDavActionButtonComponent);

function WebDavInfoCardComponent({
  title,
  value,
  detail,
  valueLarge = false,
  mono = false,
  icon: Icon,
  iconColor,
}: WebDavInfoCardProps) {
  return (
    <div
      style={{
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-input)",
      }}
    >
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 8 }}>{title}</div>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          fontSize: valueLarge ? 20 : 13,
          fontWeight: valueLarge ? 700 : 600,
          fontFamily: mono ? "'JetBrains Mono', monospace" : undefined,
          wordBreak: mono ? "break-all" : undefined,
        }}
      >
        {Icon ? <Icon size={15} style={{ color: iconColor }} /> : null}
        <span>{value}</span>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: 8 }}>{detail}</div>
    </div>
  );
}

export const WebDavInfoCard = memo(WebDavInfoCardComponent);

function WebDavSnapshotDetailsComponent({
  title,
  snapshotLabel,
  snapshotPath,
  deviceLabel,
  deviceName,
  manifestLabel,
  remoteUrl,
  copyTitle,
  canCopy,
  onCopy,
}: WebDavSnapshotDetailsProps) {
  return (
    <div
      style={{
        marginTop: 16,
        padding: "12px 14px",
        borderRadius: 10,
        background: "var(--bg-input)",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 12,
          alignItems: "center",
          flexWrap: "wrap",
          marginBottom: 8,
        }}
      >
        <div style={{ fontSize: 13, fontWeight: 600 }}>{title}</div>
        {canCopy ? (
          <button className="btn btn-ghost btn-icon-sm" onClick={onCopy} title={copyTitle}>
            <Copy size={12} />
          </button>
        ) : null}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6, fontSize: 12 }}>
        <div>
          <span style={{ color: "var(--text-muted)" }}>
            {snapshotLabel}
            {" : "}
          </span>
          <code>{snapshotPath}</code>
        </div>
        <div>
          <span style={{ color: "var(--text-muted)" }}>
            {deviceLabel}
            {" : "}
          </span>
          {deviceName}
        </div>
        <div>
          <span style={{ color: "var(--text-muted)" }}>
            {manifestLabel}
            {" : "}
          </span>
          <code style={{ wordBreak: "break-all" }}>{remoteUrl}</code>
        </div>
      </div>
    </div>
  );
}

export const WebDavSnapshotDetails = memo(WebDavSnapshotDetailsComponent);

export function detectPreset(baseUrl: string) {
  if (!baseUrl) return "custom";
  for (const preset of WEBDAV_PRESETS) {
    if (preset.matchPattern && baseUrl.includes(preset.matchPattern)) {
      return preset.id;
    }
  }
  return "custom";
}

export function formatBytes(size: number | null) {
  if (!size || size <= 0) return "0 B";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatDateTime(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}
