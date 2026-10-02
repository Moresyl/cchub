import { useId, useState } from "react";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";

type UiText = (zh: string, en: string, ja?: string) => string;
export default function ToolSearchThreshold({
  value,
  disabled,
  onSave,
  uiText,
}: {
  value: string;
  disabled: boolean;
  onSave: (value: string) => void;
  uiText: UiText;
}) {
  const id = useId();
  const current = value === "auto" ? "10" : value.slice(5);
  const [draft, setDraft] = useState(current);
  const valid = /^\d+$/.test(draft) && Number(draft) >= 0 && Number(draft) <= 100;
  return (
    <form
      className="space-y-2"
      onSubmit={(event) => {
        event.preventDefault();
        if (!disabled && valid && Number(draft) !== Number(current)) onSave(`auto:${Number(draft)}`);
      }}
    >
      <label htmlFor={id} className="text-xs text-muted-foreground">
        {uiText("工具定义占上下文的阈值（%）", "Tool definitions as a percentage of context")}
      </label>
      <div className="flex items-center gap-2">
        <Input
          id={id}
          type="number"
          min={0}
          max={100}
          step={1}
          value={draft}
          disabled={disabled}
          aria-invalid={!valid}
          className="min-w-0 flex-1"
          onChange={(event) => setDraft(event.target.value)}
        />
        <Button
          type="submit"
          variant="secondary"
          size="default"
          disabled={disabled || !valid || Number(draft) === Number(current)}
        >
          {uiText("应用阈值", "Apply threshold")}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        {uiText(
          "调整后点击应用，或按 Enter 保存；自动模式默认阈值为 10%。",
          "Apply the change or press Enter to save. Auto mode defaults to a 10% threshold.",
        )}
      </p>
    </form>
  );
}
