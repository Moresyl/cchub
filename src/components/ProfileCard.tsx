import { memo, type MouseEvent } from "react";
import { ArrowRightLeft, Check, Edit3, GripVertical, type LucideIcon } from "lucide-react";
import ProviderIcon from "./ProviderIcon";
import ProfileActionsMenu from "./ProfileActionsMenu";
import { Button } from "./ui/button";

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
  moreTitle: string;
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
      className="card card-hover profile-row"
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
        borderColor: isActive ? "var(--success)" : undefined,
        opacity: isDragging ? 0.65 : 1,
        transform: isDragOver ? "translateY(-2px)" : undefined,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
        <div style={{ display: "flex", gap: 12, minWidth: 0, flex: 1, alignItems: "center" }}>
          {reorderEnabled && (
            <Button
              variant="ghost"
              size="icon"
              className="profile-icon-button"
              type="button"
              title={text.dragEnabledTitle}
              aria-label={text.dragEnabledTitle}
              style={{ cursor: "grab" }}
            >
              <GripVertical size={14} />
            </Button>
          )}
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
          <Button
            variant={isActive ? "secondary" : "default"}
            size="sm"
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
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onEdit(profile)}
            title={text.editTitle}
            aria-label={text.editTitle}
          >
            <Edit3 size={14} />
          </Button>
          <ProfileActionsMenu
            pingLabel={text.pingTitle}
            streamLabel={text.streamTitle}
            usageLabel={text.usageTitle}
            duplicateLabel={text.duplicateTitle}
            deleteLabel={text.deleteTitle}
            moreLabel={text.moreTitle}
            isPinging={isPinging}
            isStreamChecking={isStreamChecking}
            onPing={() => onPing(profile)}
            onStreamCheck={() => onStreamCheck(profile)}
            onUsage={() => onUsage(profile)}
            onDuplicate={() => onDuplicate(profile)}
            onDelete={() => onDelete(profile)}
          />
        </div>
      </div>
    </div>
  );
}

export default memo(ProfileCardComponent);
