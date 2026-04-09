import { memo, type ChangeEventHandler, type KeyboardEventHandler } from "react";
import ToolsPermLevelLabel from "./ToolsPermLevelLabel";

export interface ToolsPermissionCardOption {
  value: number;
  label: string;
  color: string;
}

interface ToolsPermissionCardProps {
  title: string;
  currentLabel: string;
  currentDescription: string;
  currentColor: string;
  value: number;
  options: ToolsPermissionCardOption[];
  onSelect: (value: string | number) => void;
  onRangeChange: ChangeEventHandler<HTMLInputElement>;
  onRangePointerUp: () => void;
  onRangeKeyUp: KeyboardEventHandler<HTMLInputElement>;
  onRangeBlur: () => void;
}

function ToolsPermissionCardComponent({
  title,
  currentLabel,
  currentDescription,
  currentColor,
  value,
  options,
  onSelect,
  onRangeChange,
  onRangePointerUp,
  onRangeKeyUp,
  onRangeBlur,
}: ToolsPermissionCardProps) {
  return (
    <div className="card" style={{ padding: "16px 18px", display: "flex", alignItems: "center", gap: 20 }}>
      <div style={{ flex: 1 }}>
        <h4 style={{ fontSize: 13, fontWeight: 700, marginBottom: 10 }}>{title}</h4>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
          <span style={{ width: 7, height: 7, borderRadius: "50%", background: currentColor, boxShadow: `0 0 5px ${currentColor}50` }} />
          <span style={{ fontSize: 12, fontWeight: 600 }}>{currentLabel}</span>
          <span style={{ fontSize: 11, color: "var(--text-muted)" }}>— {currentDescription}</span>
        </div>
        <div style={{ position: "relative", height: 5, borderRadius: 3, background: "var(--bg-badge)" }}>
          <div
            style={{
              position: "absolute",
              left: 0,
              top: 0,
              height: "100%",
              width: `${(value / 3) * 100}%`,
              borderRadius: 3,
              background: `linear-gradient(90deg, #ef4444, ${currentColor})`,
              transition: "width 0.2s",
            }}
          />
          <input
            type="range"
            min={0}
            max={3}
            step={1}
            value={value}
            onChange={onRangeChange}
            onPointerUp={onRangePointerUp}
            onKeyUp={onRangeKeyUp}
            onBlur={onRangeBlur}
            style={{ position: "absolute", top: -8, left: 0, width: "100%", height: 22, opacity: 0, cursor: "pointer" }}
          />
          <div
            style={{
              position: "absolute",
              top: -5,
              left: `calc(${(value / 3) * 100}% - 7px)`,
              width: 14,
              height: 14,
              borderRadius: "50%",
              background: currentColor,
              border: "2px solid var(--bg-app)",
              boxShadow: `0 0 5px ${currentColor}60`,
              transition: "left 0.2s",
              pointerEvents: "none",
            }}
          />
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8 }}>
          {options.map((option) => (
            <ToolsPermLevelLabel
              key={option.value}
              level={option.value}
              label={option.label}
              color={option.color}
              active={value === option.value}
              onSelect={onSelect}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

export default memo(ToolsPermissionCardComponent);
