import { Command } from "cmdk";
import { Check, ChevronDown, Search, X } from "lucide-react";
import { memo, useCallback, useMemo, useState } from "react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";

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

function formatTokens(value: number | null | undefined): string {
  if (!value) return "";
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${Math.round(value / 1_000)}K`;
  return String(value);
}

function ModelSelectorComponent({ value, models, onChange, placeholder, disabled }: ModelSelectorProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");

  const filteredModels = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    if (!query) return models;
    return models.filter(
      (model) => model.id.toLocaleLowerCase().includes(query) || model.displayName?.toLocaleLowerCase().includes(query),
    );
  }, [models, search]);

  const trimmedSearch = search.trim();
  const canUseCustomValue =
    trimmedSearch.length > 0 &&
    !models.some((model) => model.id.toLocaleLowerCase() === trimmedSearch.toLocaleLowerCase());

  const handleOpenChange = useCallback((nextOpen: boolean) => {
    setOpen(nextOpen);
    if (!nextOpen) setSearch("");
  }, []);

  const handleSelect = useCallback(
    (nextValue: string) => {
      onChange(nextValue);
      setSearch("");
      setOpen(false);
    },
    [onChange],
  );

  if (models.length === 0) {
    return (
      <Input
        className="input model-selector-fallback"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        disabled={disabled}
      />
    );
  }

  return (
    <div className="model-selector-control">
      <Popover open={open} onOpenChange={handleOpenChange}>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="outline"
            className="model-selector-trigger"
            role="combobox"
            aria-label={placeholder || "选择模型"}
            aria-expanded={open}
            disabled={disabled}
          >
            <span className={value ? "model-selector-value" : "model-selector-placeholder"}>
              {value || placeholder || "选择模型"}
            </span>
            <ChevronDown size={13} className="model-selector-chevron" aria-hidden="true" />
          </Button>
        </PopoverTrigger>

        <PopoverContent className="model-selector-popover" onOpenAutoFocus={(event) => event.preventDefault()}>
          <Command shouldFilter={false} className="model-selector-command" label="搜索模型">
            <div className="model-selector-search">
              <Search size={14} aria-hidden="true" />
              <Command.Input
                value={search}
                onValueChange={setSearch}
                placeholder="搜索或输入模型 ID"
                aria-label="搜索模型"
                autoFocus
                onKeyDown={(event) => {
                  if (event.key === "Enter" && canUseCustomValue) {
                    event.preventDefault();
                    handleSelect(trimmedSearch);
                  }
                }}
              />
            </div>

            <Command.List className="model-selector-list">
              {filteredModels.length === 0 && !canUseCustomValue && (
                <Command.Empty className="model-selector-empty">没有匹配的模型</Command.Empty>
              )}
              {canUseCustomValue && (
                <Command.Item
                  value={`custom:${trimmedSearch}`}
                  onSelect={() => handleSelect(trimmedSearch)}
                  className="model-selector-item"
                >
                  <span className="model-selector-item-check" />
                  <span className="model-selector-item-main">
                    <span className="model-selector-item-id">使用 “{trimmedSearch}”</span>
                    <span className="model-selector-item-name">自定义模型 ID</span>
                  </span>
                </Command.Item>
              )}
              {filteredModels.map((model) => (
                <Command.Item
                  key={model.id}
                  value={model.id}
                  onSelect={() => handleSelect(model.id)}
                  className="model-selector-item"
                  aria-selected={model.id === value}
                >
                  <span className="model-selector-item-check">
                    {model.id === value && <Check size={13} aria-hidden="true" />}
                  </span>
                  <span className="model-selector-item-main">
                    <span className="model-selector-item-id">{model.id}</span>
                    {model.displayName && model.displayName !== model.id && (
                      <span className="model-selector-item-name">{model.displayName}</span>
                    )}
                  </span>
                  <ModelMeta model={model} />
                </Command.Item>
              ))}
            </Command.List>
          </Command>
        </PopoverContent>
      </Popover>

      {value && !disabled && (
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="model-selector-clear"
          onClick={() => onChange("")}
          aria-label="清除模型"
          title="清除模型"
        >
          <X size={12} aria-hidden="true" />
        </Button>
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
  return <span className="model-selector-item-meta">{parts.join(" · ")}</span>;
});

export default memo(ModelSelectorComponent);
