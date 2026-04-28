import { memo, useCallback, useMemo, useRef, useState } from "react";
import { ChevronDown, X } from "lucide-react";

export interface ModelInfo {
  id: string;
  displayName?: string | null;
  contextWindow?: number | null;
  maxOutputTokens?: number | null;
  inputPrice?: string | null;
  outputPrice?: string | null;
}

interface ModelSelectorProps {
  value: string;
  models: ModelInfo[];
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
}

function formatTokens(n: number | null | undefined): string {
  if (!n) return "";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`;
  return String(n);
}

function ModelSelectorComponent({ value, models, onChange, placeholder, disabled }: ModelSelectorProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const filtered = useMemo(() => {
    if (!search.trim()) return models;
    const q = search.toLowerCase();
    return models.filter(
      (m) => m.id.toLowerCase().includes(q) || m.displayName?.toLowerCase().includes(q),
    );
  }, [models, search]);

  const handleSelect = useCallback((id: string) => {
    onChange(id);
    setOpen(false);
    setSearch("");
  }, [onChange]);

  const handleBlur = useCallback((e: React.FocusEvent) => {
    if (!containerRef.current?.contains(e.relatedTarget as Node)) {
      setOpen(false);
      setSearch("");
    }
  }, []);

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const v = e.target.value;
    setSearch(v);
    onChange(v);
    if (!open) setOpen(true);
  }, [onChange, open]);

  if (models.length === 0) {
    return (
      <input
        className="input"
        style={{ fontSize: 12, height: 30, padding: "0 8px" }}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
      />
    );
  }

  return (
    <div ref={containerRef} style={{ position: "relative" }} onBlur={handleBlur}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          border: "1px solid var(--border-default)",
          borderRadius: 6,
          background: "var(--bg-surface)",
          height: 30,
          cursor: disabled ? "not-allowed" : "pointer",
          opacity: disabled ? 0.5 : 1,
        }}
        onClick={() => { if (!disabled) { setOpen(!open); setTimeout(() => inputRef.current?.focus(), 0); } }}
      >
        <input
          ref={inputRef}
          style={{
            flex: 1,
            border: "none",
            background: "transparent",
            outline: "none",
            fontSize: 12,
            padding: "0 8px",
            color: "var(--text-primary)",
            minWidth: 0,
          }}
          value={open ? search : value}
          onChange={handleInputChange}
          onFocus={() => { if (!open) { setOpen(true); setSearch(value); } }}
          placeholder={value || placeholder}
          disabled={disabled}
        />
        {value && !open && (
          <button
            style={{ border: "none", background: "none", cursor: "pointer", padding: "0 4px", color: "var(--text-muted)" }}
            onClick={(e) => { e.stopPropagation(); onChange(""); }}
            tabIndex={-1}
          >
            <X size={11} />
          </button>
        )}
        <ChevronDown size={12} style={{ marginRight: 6, color: "var(--text-muted)", flexShrink: 0 }} />
      </div>

      {open && (
        <div
          style={{
            position: "absolute",
            top: "calc(100% + 4px)",
            left: 0,
            right: 0,
            maxHeight: 240,
            overflowY: "auto",
            background: "var(--bg-surface)",
            border: "1px solid var(--border-default)",
            borderRadius: 6,
            zIndex: 100,
            boxShadow: "0 4px 12px rgba(0,0,0,.15)",
          }}
        >
          {filtered.length === 0 ? (
            <div style={{ padding: "8px 12px", fontSize: 11, color: "var(--text-muted)" }}>
              No matches
            </div>
          ) : (
            filtered.map((m) => (
              <div
                key={m.id}
                style={{
                  padding: "6px 10px",
                  cursor: "pointer",
                  fontSize: 12,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: 8,
                  background: m.id === value ? "var(--bg-hover)" : undefined,
                }}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleSelect(m.id)}
              >
                <div style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  <span style={{ fontWeight: m.id === value ? 600 : 400 }}>{m.id}</span>
                  {m.displayName && m.displayName !== m.id && (
                    <span style={{ marginLeft: 6, fontSize: 10, color: "var(--text-muted)" }}>{m.displayName}</span>
                  )}
                </div>
                <ModelMeta model={m} />
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}

const ModelMeta = memo(function ModelMeta({ model }: { model: ModelInfo }) {
  const parts: string[] = [];
  if (model.contextWindow) parts.push(`ctx:${formatTokens(model.contextWindow)}`);
  if (model.maxOutputTokens) parts.push(`out:${formatTokens(model.maxOutputTokens)}`);
  if (model.inputPrice) parts.push(`$${model.inputPrice}/in`);

  if (parts.length === 0) return null;

  return (
    <span style={{ fontSize: 10, color: "var(--text-muted)", whiteSpace: "nowrap", flexShrink: 0 }}>
      {parts.join(" · ")}
    </span>
  );
});

export default memo(ModelSelectorComponent);
