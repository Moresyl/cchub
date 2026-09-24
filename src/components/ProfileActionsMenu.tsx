import { Activity, Copy, Gauge, MoreHorizontal, Trash2, Wifi } from "lucide-react";
import { useState } from "react";
import { Button } from "./ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface ProfileActionsMenuProps {
  pingLabel: string;
  streamLabel: string;
  usageLabel: string;
  duplicateLabel: string;
  deleteLabel: string;
  moreLabel: string;
  isPinging: boolean;
  isStreamChecking: boolean;
  onPing: () => void;
  onStreamCheck: () => void;
  onUsage: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
}

export default function ProfileActionsMenu({
  pingLabel,
  streamLabel,
  usageLabel,
  duplicateLabel,
  deleteLabel,
  moreLabel,
  isPinging,
  isStreamChecking,
  onPing,
  onStreamCheck,
  onUsage,
  onDuplicate,
  onDelete,
}: ProfileActionsMenuProps) {
  const [open, setOpen] = useState(false);

  const run = (action: () => void) => {
    setOpen(false);
    action();
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          type="button"
          aria-label={moreLabel}
          title={moreLabel}
          aria-haspopup="menu"
          aria-expanded={open}
        >
          <MoreHorizontal size={16} />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="profile-card-menu" align="end" role="menu">
        <Button
          variant="ghost"
          className="profile-card-menu-item"
          role="menuitem"
          disabled={isPinging}
          onClick={() => run(onPing)}
        >
          {isPinging ? <span className="spinner size-[13px]" /> : <Activity size={15} />}
          {pingLabel}
        </Button>
        <Button
          variant="ghost"
          className="profile-card-menu-item"
          role="menuitem"
          disabled={isStreamChecking}
          onClick={() => run(onStreamCheck)}
        >
          {isStreamChecking ? <span className="spinner size-[13px]" /> : <Wifi size={15} />}
          {streamLabel}
        </Button>
        <Button variant="ghost" className="profile-card-menu-item" role="menuitem" onClick={() => run(onUsage)}>
          <Gauge size={15} />
          {usageLabel}
        </Button>
        <Button variant="ghost" className="profile-card-menu-item" role="menuitem" onClick={() => run(onDuplicate)}>
          <Copy size={15} />
          {duplicateLabel}
        </Button>
        <div className="profile-card-menu-separator" role="separator" />
        <Button
          variant="ghost"
          className="profile-card-menu-item profile-card-menu-item-danger"
          role="menuitem"
          onClick={() => run(onDelete)}
        >
          <Trash2 size={15} />
          {deleteLabel}
        </Button>
      </PopoverContent>
    </Popover>
  );
}
