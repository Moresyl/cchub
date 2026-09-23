import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Activity, Copy, Gauge, MoreHorizontal, Trash2, Wifi } from "lucide-react";
import { Button } from "./ui/button";

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
  const wrapperRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [menuPosition, setMenuPosition] = useState({ top: 0, left: 0 });
  const menuId = useId();

  useEffect(() => {
    if (!open) return;

    const closeOnOutsidePointer = (event: Event) => {
      const target = event.target as Node;
      if (!wrapperRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    const closeMenu = () => setOpen(false);

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("mousedown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    window.addEventListener("resize", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("mousedown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
      window.removeEventListener("resize", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
    };
  }, [open]);

  const run = (action: () => void) => {
    setOpen(false);
    action();
  };

  const toggleMenu = () => {
    if (open) {
      setOpen(false);
      return;
    }
    const rect = triggerRef.current?.getBoundingClientRect();
    if (!rect) return;
    const menuWidth = 190;
    const menuHeight = 214;
    const opensUpward = rect.bottom + 6 + menuHeight > window.innerHeight;
    const maxLeft = Math.max(8, window.innerWidth - menuWidth - 8);
    setMenuPosition({
      top: Math.max(8, opensUpward ? rect.top - menuHeight - 6 : rect.bottom + 6),
      left: Math.min(maxLeft, Math.max(8, rect.right - menuWidth)),
    });
    setOpen(true);
  };

  return (
    <div className="profile-card-menu-wrap" ref={wrapperRef}>
      <Button
        ref={triggerRef}
        variant="ghost"
        size="icon"
        type="button"
        aria-label={moreLabel}
        title={moreLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={toggleMenu}
      >
        <MoreHorizontal size={16} />
      </Button>
      {open &&
        createPortal(
          <div
            className="profile-card-menu"
            id={menuId}
            role="menu"
            ref={menuRef}
            style={{ top: menuPosition.top, left: menuPosition.left }}
          >
            <Button
              variant="ghost"
              className="profile-card-menu-item"
              role="menuitem"
              disabled={isPinging}
              onClick={() => run(onPing)}
            >
              {isPinging ? <span className="spinner" style={{ width: 13, height: 13 }} /> : <Activity size={15} />}
              {pingLabel}
            </Button>
            <Button
              variant="ghost"
              className="profile-card-menu-item"
              role="menuitem"
              disabled={isStreamChecking}
              onClick={() => run(onStreamCheck)}
            >
              {isStreamChecking ? <span className="spinner" style={{ width: 13, height: 13 }} /> : <Wifi size={15} />}
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
          </div>,
          document.body,
        )}
    </div>
  );
}
