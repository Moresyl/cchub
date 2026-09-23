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
      className={`profile-row ${isActive ? "profile-row-active" : ""} ${isDragging ? "profile-row-dragging" : ""} ${isDragOver ? "profile-row-drag-over" : ""}`}
      draggable={reorderEnabled}
      onDragStart={() => onDragStart(profile.id)}
      onDragEnter={() => onDragEnter(profile.id)}
      onDragOver={(event) => {
        if (!reorderEnabled) return;
        event.preventDefault();
      }}
      onDragEnd={onDragEnd}
      onDrop={handleDrop}
    >
      <div className="profile-row-layout">
        <div className="profile-row-main">
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
          <div className="profile-row-icon">
            <ProviderIcon iconUrl={iconUrl} fallbackIcon={Icon} size={16} />
          </div>
          <div className="profile-row-copy">
            <div className="profile-row-title-line">
              <span className="profile-row-name">{profile.name}</span>
              <span className="badge badge-muted profile-row-tool">{toolTag}</span>
              {isActive && <span className="badge badge-success">{text.activeTag}</span>}
              {sharedCount > 1 && <span className="badge badge-accent">{sharedTag}</span>}
              {ping && (
                <span className={`badge ${pingTone}`}>
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
                <span className={`badge ${streamTone}`}>
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
            <div className="profile-row-meta">
              {baseUrl && <span className="profile-row-url">{baseUrl}</span>}
              {model && <span className="profile-row-model">{model}</span>}
              {!baseUrl && !model && <span>{formatTime(profile.updated_at || profile.created_at)}</span>}
            </div>
          </div>
        </div>

        <div className="card-actions profile-row-actions">
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
