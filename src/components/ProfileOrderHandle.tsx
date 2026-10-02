import { ArrowDown, ArrowDownToLine, ArrowUp, ArrowUpToLine, GripVertical } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import { getLocale } from "../lib/i18n";
import { profileMoveTarget, type ProfileMoveDirection } from "../lib/profileOrdering";
import { Button } from "./ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

interface ProfileOrderHandleProps {
  id: string;
  name: string;
  position: number;
  count: number;
  busy: boolean;
  onMove: (id: string, direction: ProfileMoveDirection) => void;
  onDragStart: (id: string) => void;
  onDragEnd: () => void;
}

export default function ProfileOrderHandle({
  id,
  name,
  position,
  count,
  busy,
  onMove,
  onDragStart,
  onDragEnd,
}: ProfileOrderHandleProps) {
  const locale = getLocale();
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const keepFocus = useRef(false);
  useLayoutEffect(() => {
    if (keepFocus.current) {
      triggerRef.current?.focus({ preventScroll: true });
      triggerRef.current?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
      keepFocus.current = false;
    }
  }, [position]);
  const move = (direction: ProfileMoveDirection) => {
    if (busy || profileMoveTarget(position, count, direction) === null) return;
    keepFocus.current = true;
    setOpen(false);
    onMove(id, direction);
  };
  const actions = [
    { direction: "first", icon: ArrowUpToLine, label: text("移到最前", "Move to first", "先頭に移動") },
    { direction: "up", icon: ArrowUp, label: text("上移", "Move up", "上に移動") },
    { direction: "down", icon: ArrowDown, label: text("下移", "Move down", "下に移動") },
    { direction: "last", icon: ArrowDownToLine, label: text("移到最后", "Move to last", "末尾に移動") },
  ] as const;
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          ref={triggerRef}
          type="button"
          variant="ghost"
          size="icon"
          className="profile-icon-button"
          data-profile-order={id}
          aria-label={text(`调整“${name}”的顺序`, `Reorder “${name}”`, `「${name}」の並び順を変更`)}
          aria-keyshortcuts="Alt+ArrowUp Alt+ArrowDown Alt+Home Alt+End"
          aria-haspopup="menu"
          aria-expanded={open}
          aria-disabled={busy}
          title={text(
            "拖动调整顺序，或按 Alt+↑/↓；点击查看更多操作",
            "Drag to reorder, or press Alt+↑/↓; click for more options",
            "ドラッグまたは Alt+↑/↓ で移動。クリックでその他の操作",
          )}
          draggable={!busy}
          style={{ cursor: busy ? "default" : "grab" }}
          onDragStart={(event) => {
            if (busy) {
              event.preventDefault();
              return;
            }
            setOpen(false);
            event.dataTransfer.effectAllowed = "move";
            event.dataTransfer.setData("text/plain", id);
            onDragStart(id);
          }}
          onDragEnd={onDragEnd}
          onKeyDown={(event) => {
            if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || event.nativeEvent.isComposing)
              return;
            const direction =
              event.key === "ArrowUp"
                ? "up"
                : event.key === "ArrowDown"
                  ? "down"
                  : event.key === "Home"
                    ? "first"
                    : event.key === "End"
                      ? "last"
                      : null;
            if (!direction) return;
            event.preventDefault();
            event.stopPropagation();
            move(direction);
          }}
          onClick={(event) => {
            if (busy) event.preventDefault();
          }}
        >
          <GripVertical size={14} aria-hidden="true" />
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="profile-card-menu"
        role="menu"
        aria-label={text("配置排序", "Configuration order", "設定の並び順")}
        onKeyDown={(event) => {
          if (
            !["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) ||
            event.altKey ||
            event.ctrlKey ||
            event.metaKey
          )
            return;
          const items = Array.from(
            event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)'),
          );
          if (!items.length) return;
          event.preventDefault();
          const index = items.indexOf(document.activeElement as HTMLButtonElement);
          const next =
            event.key === "Home"
              ? 0
              : event.key === "End"
                ? items.length - 1
                : (index + (event.key === "ArrowUp" ? -1 : 1) + items.length) % items.length;
          items[next].focus();
        }}
      >
        {actions.map(({ direction, icon: Icon, label }) => (
          <Button
            key={direction}
            type="button"
            variant="ghost"
            role="menuitem"
            className="profile-card-menu-item"
            disabled={busy || profileMoveTarget(position, count, direction) === null}
            onClick={() => move(direction)}
          >
            <Icon size={15} aria-hidden="true" />
            {label}
          </Button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
