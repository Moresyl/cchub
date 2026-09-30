import { cloneElement, memo, useId, type ReactElement } from "react";
import { RefreshCw } from "lucide-react";
import ModelSelector from "../../components/ModelSelector";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { discoveredModelFields, selectProfileModel } from "../../lib/configProfiles/modelSelection";
import type { StructuredDraftFields } from "../../lib/configProfiles";
import type { ModelInfo } from "../../lib/modelCatalog";
import { supportsModelFetch } from "./helpers";

interface ProfileModelsSectionProps {
  locale: string;
  localeText: (zh: string, en: string, ja?: string) => string;
  draftTool: string;
  draftFields: StructuredDraftFields;
  fetchedModels: string[];
  fetchedModelDetails: ModelInfo[];
  fetchingModels: boolean;
  modelFetchError: string | null;
  onFetchModels: () => void;
  onDraftChange: (toolId: string, next: Partial<StructuredDraftFields>) => void;
}

function Field({ label, children }: { label: string; children: ReactElement<{ id?: string }> }) {
  const id = useId();
  return (
    <div className="profile-model-field">
      <label htmlFor={id} className="field-label">
        {label}
      </label>
      {cloneElement(children, { id })}
    </div>
  );
}

export const ProfileModelsSection = memo(function ProfileModelsSection({
  locale,
  localeText,
  draftTool,
  draftFields: fields,
  fetchedModels,
  fetchedModelDetails,
  fetchingModels,
  modelFetchError,
  onFetchModels,
  onDraftChange,
}: ProfileModelsSectionProps) {
  const selected = fetchedModelDetails.find((model) => model.id === fields.model);
  const nativeFields = selected ? discoveredModelFields(draftTool, selected) : {};
  const hasCapabilities =
    selected &&
    [
      selected.contextWindow,
      selected.maxOutputTokens,
      selected.nativeEndpoints?.length,
      selected.supportedReasoningLevels?.length,
      selected.inputModalities?.length,
      selected.outputModalities?.length,
    ].some(Boolean);
  const input = (
    key: "model" | "reasoningModel" | "haikuModel" | "sonnetModel" | "opusModel",
    label: string,
    placeholder: string,
  ) => (
    <Field label={label}>
      <ModelSelector
        value={fields[key]}
        models={fetchedModelDetails}
        label={label}
        placeholder={placeholder}
        onChange={(value) =>
          onDraftChange(draftTool, key === "model" ? selectProfileModel(draftTool, fields, value) : { [key]: value })
        }
      />
    </Field>
  );
  const tokenNumber = (value: number) =>
    value.toLocaleString(locale === "zh" ? "zh-CN" : locale === "ja" ? "ja-JP" : "en-US");
  return (
    <section className="profile-model-section" aria-label={localeText("模型配置", "Models", "モデル設定")}>
      <div className="profile-model-header">
        <h3>{localeText("模型配置", "Models", "モデル設定")}</h3>
        {supportsModelFetch(draftTool) && (
          <Button type="button" variant="secondary" onClick={onFetchModels} disabled={fetchingModels}>
            {fetchingModels ? <span className="spinner" /> : <RefreshCw size={14} />}
            {localeText("拉取模型列表", "Fetch models", "モデル一覧を取得")}
          </Button>
        )}
      </div>
      {(modelFetchError || fetchedModels.length > 0) && (
        <p
          className={`profile-model-notice${modelFetchError ? " is-error" : ""}`}
          role={modelFetchError ? "alert" : "status"}
        >
          {modelFetchError ||
            localeText(
              `${fetchedModels.length} 个可选模型`,
              `${fetchedModels.length} available models`,
              `${fetchedModels.length} 個のモデル`,
            )}
        </p>
      )}
      <div className="profile-model-grid">
        {draftTool === "claude" ? (
          <>
            {input("model", localeText("主模型", "Main model", "メインモデル"), "claude-sonnet-4-5")}
            {input("reasoningModel", localeText("推理模型", "Reasoning model", "推論モデル"), "claude-sonnet-4-5")}
            {input(
              "haikuModel",
              localeText("Haiku 默认模型", "Default Haiku", "Haiku の既定モデル"),
              "claude-haiku-3-5",
            )}
            {input(
              "sonnetModel",
              localeText("Sonnet 默认模型", "Default Sonnet", "Sonnet の既定モデル"),
              "claude-sonnet-4-5",
            )}
            {input("opusModel", localeText("Opus 默认模型", "Default Opus", "Opus の既定モデル"), "claude-opus-5")}
          </>
        ) : (
          <>
            {input(
              "model",
              localeText("模型 ID", "Model ID", "モデル ID"),
              localeText("输入或选择模型 ID", "Enter or select a model ID", "モデル ID を入力または選択"),
            )}
            <Field label={localeText("模型显示名", "Display name", "表示名")}>
              <Input
                value={fields.modelName}
                onChange={(event) => onDraftChange(draftTool, { modelName: event.target.value })}
                placeholder={localeText("可选，默认同 ID", "Optional, defaults to ID", "任意、既定は ID")}
              />
            </Field>
            {draftTool === "opencode" && (
              <>
                <Field label={localeText("上下文上限", "Context limit", "コンテキスト上限")}>
                  <Input
                    inputMode="numeric"
                    value={fields.openCodeContextLimit}
                    onChange={(event) => onDraftChange(draftTool, { openCodeContextLimit: event.target.value })}
                    placeholder={localeText("使用工具默认值", "Use tool default", "ツールの既定値を使用")}
                  />
                </Field>
                <Field label={localeText("输出上限", "Output limit", "出力上限")}>
                  <Input
                    inputMode="numeric"
                    value={fields.openCodeOutputLimit}
                    onChange={(event) => onDraftChange(draftTool, { openCodeOutputLimit: event.target.value })}
                    placeholder={localeText("使用工具默认值", "Use tool default", "ツールの既定値を使用")}
                  />
                </Field>
                <Field label={localeText("输入模态", "Input modalities", "入力モダリティ")}>
                  <Input
                    value={fields.openCodeInputModalities}
                    placeholder="text,image,pdf"
                    onChange={(event) => onDraftChange(draftTool, { openCodeInputModalities: event.target.value })}
                  />
                </Field>
                <Field label={localeText("输出模态", "Output modalities", "出力モダリティ")}>
                  <Input
                    value={fields.openCodeOutputModalities}
                    placeholder="text"
                    onChange={(event) => onDraftChange(draftTool, { openCodeOutputModalities: event.target.value })}
                  />
                </Field>
              </>
            )}
          </>
        )}
      </div>
      {hasCapabilities && (
        <div className="profile-model-capabilities">
          <div className="profile-model-capabilities-header">
            <span>
              {localeText("供应商报告的模型能力", "Provider-reported capabilities", "プロバイダーが報告した機能")}
            </span>
            {Object.keys(nativeFields).length > 0 && (
              <Button type="button" variant="secondary" onClick={() => onDraftChange(draftTool, nativeFields)}>
                {localeText("填入当前模型", "Apply to current model", "現在のモデルに適用")}
              </Button>
            )}
          </div>
          <dl>
            {selected.contextWindow && (
              <div>
                <dt>{localeText("上下文", "Context", "コンテキスト")}</dt>
                <dd>{tokenNumber(selected.contextWindow)} tokens</dd>
              </div>
            )}
            {selected.maxOutputTokens && (
              <div>
                <dt>{localeText("最大输出", "Maximum output", "最大出力")}</dt>
                <dd>{tokenNumber(selected.maxOutputTokens)} tokens</dd>
              </div>
            )}
            {!!selected.supportedReasoningLevels?.length && (
              <div>
                <dt>{localeText("推理等级", "Reasoning levels", "推論レベル")}</dt>
                <dd>{selected.supportedReasoningLevels.join(" · ")}</dd>
              </div>
            )}
            {!!selected.inputModalities?.length && (
              <div>
                <dt>{localeText("输入", "Input", "入力")}</dt>
                <dd>{selected.inputModalities.join(" · ")}</dd>
              </div>
            )}
            {!!selected.outputModalities?.length && (
              <div>
                <dt>{localeText("输出", "Output", "出力")}</dt>
                <dd>{selected.outputModalities.join(" · ")}</dd>
              </div>
            )}
            {!!selected.nativeEndpoints?.length && (
              <div>
                <dt>{localeText("原生端点", "Native endpoints", "ネイティブエンドポイント")}</dt>
                <dd>{selected.nativeEndpoints.join(" · ")}</dd>
              </div>
            )}
          </dl>
        </div>
      )}
    </section>
  );
});
