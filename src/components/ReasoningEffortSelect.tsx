import { useId } from "react";
import { codexReasoningChoices } from "../lib/codexReasoning";
import { SimpleSelect } from "./ui/simple-select";

interface Props {
  id?: string;
  value: string;
  reportedLevels?: string[] | null;
  defaultEffort?: string | null;
  onValueChange: (value: string) => void;
  localeText: (zh: string, en: string, ja?: string) => string;
}

export default function ReasoningEffortSelect({
  id,
  value,
  reportedLevels,
  defaultEffort,
  onValueChange,
  localeText,
}: Props) {
  const descriptionId = useId();
  const { reported, levels, unsupported } = codexReasoningChoices(value, reportedLevels);
  const options = [
    {
      value: "",
      label: defaultEffort
        ? localeText(
            `使用模型默认值（${defaultEffort}）`,
            `Use model default (${defaultEffort})`,
            `モデルの既定値を使用（${defaultEffort}）`,
          )
        : localeText("使用模型默认值", "Use model default", "モデルの既定値を使用"),
    },
    ...levels.map((level) => ({ value: level, label: level })),
  ];
  if (value && !levels.includes(value)) {
    options.push({
      value,
      label: unsupported
        ? localeText(
            `${value}（原配置，未报告支持）`,
            `${value} (saved, not reported as supported)`,
            `${value}（保存済み、対応未確認）`,
          )
        : value,
    });
  }
  return (
    <div className="min-w-0 space-y-2">
      <SimpleSelect
        id={id ?? `${descriptionId}-control`}
        value={value}
        ariaLabel={localeText("推理强度", "Reasoning effort", "推論強度")}
        ariaDescribedBy={descriptionId}
        options={options.map((option) => ({ ...option, disabled: unsupported && option.value === value }))}
        disabled={reported && levels.length === 0 && value === ""}
        onValueChange={onValueChange}
      />
      <p
        id={descriptionId}
        className={`break-words text-xs ${unsupported ? "text-[var(--warning)]" : "text-muted-foreground"}`}
      >
        {unsupported
          ? localeText(
              "原配置的推理强度未被当前模型报告支持。请选择支持的等级或模型默认值；未操作前会保留原配置。",
              "The model does not report support for the saved effort. Choose a supported level or model default; the saved value is preserved until you change it.",
              "現在のモデルは保存済みの推論強度への対応を報告していません。対応レベルか既定値を選択してください。変更するまで元の値を保持します。",
            )
          : reported
            ? levels.length
              ? localeText(
                  "可选等级来自当前模型的能力信息。",
                  "Levels are reported by the current model.",
                  "現在のモデルが報告したレベルです。",
                )
              : localeText(
                  "当前模型明确未提供可配置的推理等级，使用模型默认行为。",
                  "The model explicitly reports no configurable reasoning levels. Its default behavior is used.",
                  "このモデルには設定可能な推論レベルがありません。既定の動作を使用します。",
                )
            : localeText(
                "尚未获取当前模型的推理等级；可保留原值或使用模型默认值。",
                "Reasoning levels are not reported yet; keep the saved value or use model default.",
                "推論レベルはまだ取得されていません。保存済みの値か既定値を使用できます。",
              )}
      </p>
    </div>
  );
}
