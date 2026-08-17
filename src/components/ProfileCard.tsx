import { memo, type MouseEvent } from "react";
import {
  Activity,
  ArrowRightLeft,
  Check,
  Copy,
  Edit3,
  Gauge,
  GripVertical,
  Trash2,
  Wifi,
  type LucideIcon,
} from "lucide-react";
import ProviderIcon from "./ProviderIcon";

interface ConfigProfileCard {
  id: string;
  name: string;
  tool_id: string;
  config_snapshot: string;
  sort_order: number;
  source_type?: string | null;
  source_key?: string | null;
  created_at: string | null;
  updated_at: string | null;
}

interface ProviderStatus {
  status: string;
  latency_ms: number | null;
}

interface ProfileCardText {
  activeTag: string;
  pingFast: string;
  pingMedium: string;
  pingSlow: string;
  pingError: string;
  streamHealthy: string;
  streamReachable: string;
  streamUnsupported: string;
  streamUnconfigured: string;
  streamError: string;
  dragEnabledTitle: string;
  dragDisabledTitle: string;
  pingTitle: string;
  streamTitle: string;
  usageTitle: string;
  duplicateTitle: string;
  editTitle: string;
  deleteTitle: string;
  activeButton: string;
  applyButton: string;
}

interface ProfileCardProps {
  profile: ConfigProfileCard;
  icon: LucideIcon;
  isActive: boolean;
  toolTag: string;
  iconUrl?: string;
  sharedTag: string;
  baseUrl?: string;
  model?: string;
  sharedCount: number;
  ping?: ProviderStatus;
  pingTone: string;
  streamCheck?: ProviderStatus;
  streamTone: string;
  reorderEnabled: boolean;
  isDragging: boolean;
  isDragOver: boolean;
  isPinging: boolean;
  isStreamChecking: boolean;
  isApplying: boolean;
  text: ProfileCardText;
  onDragStart: (profileId: string) => void;
  onDragEnter: (profileId: string) => void;
  onDragEnd: () => void;
  onDrop: (profileId: string) => void;
  onPing: (profile: ConfigProfileCard) => void;
  onStreamCheck: (profile: ConfigProfileCard) => void;
  onUsage: (profile: ConfigProfileCard) => void;
  onApply: (profile: ConfigProfileCard) => void;
  onDuplicate: (profile: ConfigProfileCard) => void;
  onEdit: (profile: ConfigProfileCard) => void;
  onDelete: (profile: ConfigProfileCard) => void;
}

function formatTime(value: string | null) {
  if (!value) return "";
  return value.replace("T", " ").slice(0, 19);
}

function ProfileCardComponent({
  profile,
  icon: Icon,
  isActive,
  toolTag,
  iconUrl,
  sharedTag,
  baseUrl,
  model,
  sharedCount,
  ping,
  pingTone,
  streamCheck,
  streamTone,
  reorderEnabled,
  isDragging,
  isDragOver,
  isPinging,
  isStreamChecking,
  isApplying,
  text,
  onDragStart,
  onDragEnter,
  onDragEnd,
  onDrop,
  onPing,
  onStreamCheck,
  onUsage,
  onApply,
  onDuplicate,
  onEdit,
  onDelete,
}: ProfileCardProps) {
  const handleDrop = (event: MouseEvent<HTMLDivElement> | React.DragEvent<HTMLDivElement>) => {
    if (!reorderEnabled) return;
    event.preventDefault();
    onDrop(profile.id);
  };

  return (
    <div
      className="card card-hover"
      draggable={reorderEnabled}
      onDragStart={() => onDragStart(profile.id)}
      onDragEnter={() => onDragEnter(profile.id)}
      onDragOver={(event) => {
        if (!reorderEnabled) return;
        event.preventDefault();
      }}
      onDragEnd={onDragEnd}
      onDrop={handleDrop}
      style={{
        padding: "16px 18px",
        borderColor: isActive ? "var(--success)" : undefined,
        boxShadow: isActive ? "0 0 0 1px color-mix(in srgb, var(--success) 30%, transparent)" : undefined,
        opacity: isDragging ? 0.65 : 1,
        transform: isDragOver ? "translateY(-2px)" : undefined,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div style={{ display: "flex", gap: 12, minWidth: 0, flex: 1, alignItems: "center" }}>
          <button
            className="btn btn-ghost btn-icon-sm"
            type="button"
            title={reorderEnabled ? text.dragEnabledTitle : text.dragDisabledTitle}
            style={{ cursor: reorderEnabled ? "grab" : "default", opacity: reorderEnabled ? 1 : 0.45 }}
          >
            <GripVertical size={14} />
          </button>
          <div className="icon-box" style={{ width: 36, height: 36, borderRadius: 8, flexShrink: 0 }}>
            <ProviderIcon iconUrl={iconUrl} fallbackIcon={Icon} size={16} />
          </div>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <span style={{ fontSize: 14, fontWeight: 600 }}>{profile.name}</span>
              <span className="badge badge-muted" style={{ textTransform: "capitalize", fontSize: 10 }}>
                {toolTag}
              </span>
              {isActive && (
                <span className="badge badge-success" style={{ fontSize: 10 }}>
                  {text.activeTag}
                </span>
              )}
              {sharedCount > 1 && (
                <span className="badge badge-accent" style={{ fontSize: 10 }}>
                  {sharedTag}
                </span>
              )}
              {ping && (
                <span className={`badge ${pingTone}`} style={{ fontSize: 10 }}>
                  {ping.status === "fast"
                    ? text.pingFast
                    : ping.status === "medium"
                      ? text.pingMedium
                      : ping.status === "slow"
                        ? text.pingSlow
                        : text.pingError}
                  {ping.latency_ms != null ? ` · ${ping.latency_ms}ms` : ""}
                </span>
              )}
              {streamCheck && (
                <span className={`badge ${streamTone}`} style={{ fontSize: 10 }}>
                  {streamCheck.status === "healthy"
                    ? text.streamHealthy
                    : streamCheck.status === "reachable"
                      ? text.streamReachable
                      : streamCheck.status === "unsupported"
                        ? text.streamUnsupported
                        : streamCheck.status === "unconfigured"
                          ? text.streamUnconfigured
                          : text.streamError}
                  {streamCheck.latency_ms != null ? ` · ${streamCheck.latency_ms}ms` : ""}
                </span>
              )}
            </div>
            <div style={{ display: "flex", gap: 16, marginTop: 4, fontSize: 12, color: "var(--text-muted)" }}>
              {baseUrl && (
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 260 }}>
                  {baseUrl}
                </span>
              )}
              {model && <span style={{ flexShrink: 0 }}>{model}</span>}
              {!baseUrl && !model && <span>{formatTime(profile.updated_at || profile.created_at)}</span>}
            </div>
          </div>
        </div>

        <div className="card-actions" style={{ display: "flex", alignItems: "center", gap: 6, flexShrink: 0 }}>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onPing(profile)} title={text.pingTitle}>
            {isPinging ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Activity size={14} />}
          </button>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onStreamCheck(profile)} title={text.streamTitle}>
            {isStreamChecking ? <div className="spinner" style={{ width: 12, height: 12 }} /> : <Wifi size={14} />}
          </button>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onUsage(profile)} title={text.usageTitle}>
            <Gauge size={14} />
          </button>
          <button
            className={`btn btn-xs ${isActive ? "btn-secondary" : "btn-primary"}`}
            onClick={() => onApply(profile)}
            disabled={isApplying}
            style={{ gap: 5 }}
          >
            {isApplying ? (
              <div className="spinner" style={{ width: 11, height: 11 }} />
            ) : isActive ? (
              <Check size={11} />
            ) : (
              <ArrowRightLeft size={11} />
            )}
            {isActive ? text.activeButton : text.applyButton}
          </button>
          <button
            className="btn btn-ghost btn-icon-sm"
            onClick={() => onDuplicate(profile)}
            title={text.duplicateTitle}
          >
            <Copy size={14} />
          </button>
          <button className="btn btn-ghost btn-icon-sm" onClick={() => onEdit(profile)} title={text.editTitle}>
            <Edit3 size={14} />
          </button>
          <button
            className="btn btn-danger-ghost btn-icon-sm"
            onClick={() => onDelete(profile)}
            title={text.deleteTitle}
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}

export default memo(ProfileCardComponent);
