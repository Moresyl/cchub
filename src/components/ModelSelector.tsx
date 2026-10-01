import { Command } from "cmdk";
import { Check, ChevronDown, Search, X } from "lucide-react";
import { memo, useCallback, useMemo, useState } from "react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";
import type { ModelInfo } from "../lib/modelCatalog";
import ModelBillingBadge from "./ModelBillingBadge";
import { getLocale } from "../lib/i18n";

export type { ModelInfo } from "../lib/modelCatalog";

interface ModelSelectorProps {
  value: string;
  models: ModelInfo[];
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  label?: string;
  id?: string;
}

function formatTokens(value: number | null | undefined): string {
  if (!value) return "";
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${Math.round(value / 1_000)}K`;
  return String(value);
}

function ModelSelectorComponent({ value, models, onChange, placeholder, disabled, label, id }: ModelSelectorProps) {
  const locale = getLocale();
  const text = (zh: string, en: string, ja: string) => (locale === "zh" ? zh : locale === "ja" ? ja : en);
  const chooseLabel = text("选择模型", "Choose model", "モデルを選択");
  const searchLabel = text("搜索模型", "Search models", "モデルを検索");
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
        id={id}
        className="input model-selector-fallback"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        aria-label={label || placeholder || chooseLabel}
      />
    );
  }

  return (
    <div className="model-selector-control">
      <Popover open={open} onOpenChange={handleOpenChange}>
        <PopoverTrigger asChild>
          <Button
            id={id}
            type="button"
            variant="outline"
            className="model-selector-trigger"
            role="combobox"
            aria-label={label || placeholder || chooseLabel}
            aria-expanded={open}
            disabled={disabled}
          >
            <span className={value ? "model-selector-value" : "model-selector-placeholder"}>
              {value || placeholder || chooseLabel}
            </span>
            <ChevronDown size={13} className="model-selector-chevron" aria-hidden="true" />
          </Button>
        </PopoverTrigger>

        <PopoverContent className="model-selector-popover" onOpenAutoFocus={(event) => event.preventDefault()}>
          <Command defaultValue={value} shouldFilter={false} className="model-selector-command" label={searchLabel}>
            <div className="model-selector-search">
              <Search size={14} aria-hidden="true" />
              <Command.Input
                value={search}
                onValueChange={setSearch}
                placeholder={text("搜索或输入模型 ID", "Search or enter a model ID", "モデル ID を検索または入力")}
                aria-label={searchLabel}
                autoFocus
                onKeyDown={(event) => {
                  if (event.key === "Enter" && (event.nativeEvent.isComposing || event.keyCode === 229)) {
                    event.preventDefault();
                    event.stopPropagation();
                    return;
                  }
                  if (event.key === "Enter" && canUseCustomValue && filteredModels.length === 0) {
                    event.preventDefault();
                    handleSelect(trimmedSearch);
                  }
                }}
              />
            </div>

            <Command.List className="model-selector-list">
              {filteredModels.length === 0 && !canUseCustomValue && (
                <Command.Empty className="model-selector-empty">
                  {text("没有匹配的模型", "No matching models", "一致するモデルがありません")}
                </Command.Empty>
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
                  <span
                    className={`model-selector-item-main${model.premiumRequestBilling ? " model-selector-item-main-with-billing" : ""}`}
                  >
                    <span className="model-selector-item-id" title={model.id}>
                      {model.id}
                    </span>
                    {((model.displayName && model.displayName !== model.id) || model.premiumRequestBilling) && (
                      <span className="flex min-w-0 max-w-full flex-wrap items-center gap-2">
                        {model.displayName && model.displayName !== model.id && (
                          <span className="model-selector-item-name">{model.displayName}</span>
                        )}
                        {model.premiumRequestBilling && <ModelBillingBadge value={model.premiumRequestBilling} />}
                      </span>
                    )}
                  </span>
                  <ModelMeta model={model} />
                </Command.Item>
              ))}
              {canUseCustomValue && (
                <Command.Item
                  value={`custom:${trimmedSearch}`}
                  onSelect={() => handleSelect(trimmedSearch)}
                  className="model-selector-item"
                >
                  <span className="model-selector-item-check" />
                  <span className="model-selector-item-main">
                    <span className="model-selector-item-id">
                      {text(`使用 “${trimmedSearch}”`, `Use “${trimmedSearch}”`, `「${trimmedSearch}」を使用`)}
                    </span>
                    <span className="model-selector-item-name">
                      {text("自定义模型 ID", "Custom model ID", "カスタムモデル ID")}
                    </span>
                  </span>
                </Command.Item>
              )}
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
          aria-label={text("清除模型", "Clear model", "モデルをクリア")}
          title={text("清除模型", "Clear model", "モデルをクリア")}
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
  if (model.outputPrice) parts.push(`$${model.outputPrice}/out`);
  if (parts.length === 0) return null;
  return <span className="model-selector-item-meta">{parts.join(" · ")}</span>;
});

export default memo(ModelSelectorComponent);
