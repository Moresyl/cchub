import type { StructuredDraftFields } from "../../lib/configProfiles";
import { parseModelAliases, previewModelAlias, validateModelAliases } from "../../lib/configProfiles/modelAliases";

type LocaleText = (zh: string, en: string, ja?: string) => string;
type AliasFields = Pick<StructuredDraftFields, "localProxyModelAliases" | "localProxyModelAliasesRaw">;

export function modelAliasValidationMessage(fields: AliasFields, t: LocaleText, model?: string): string | null {
  const rows = fields.localProxyModelAliases ?? [];
  const error = validateModelAliases(rows, fields.localProxyModelAliasesRaw);
  if (error?.kind === "shape") {
    return t(
      "导入的别名格式损坏，已保留原始数据。请在原始配置中修复，或明确清空后重新填写。",
      "Imported aliases are malformed. Repair the raw configuration or explicitly clear them.",
      "別名の形式が不正です。元の設定で修正するか、明示的にクリアしてください。",
    );
  }
  if (error?.kind === "limit")
    return t("最多支持 128 条别名。", "Up to 128 aliases are supported.", "別名は最大 128 件です。");
  if (error?.kind === "duplicate") {
    return t(
      `第 ${error.row} 条请求模型重复，请合并或删除。`,
      `Requested model in row ${error.row} is duplicated.`,
      `${error.row} 行目のリクエストモデルが重複しています。`,
    );
  }
  if (error) {
    return t(
      `请完整填写第 ${error.row} 条：名称限 1024 字节，不支持空格或查询符号；请求模型仅支持完整名称或 *。`,
      `Complete row ${error.row}: names must be within 1024 bytes, without spaces or query characters; use an exact requested model or *.`,
      `${error.row} 行目を入力してください。名前は 1024 バイト以内、空白やクエリ記号は使用できません。`,
    );
  }
  if (model && previewModelAlias(rows, model) === null) {
    return t(
      "当前模型展开后的别名无效或超过 1024 字节。",
      "The expanded alias for the current model is invalid or exceeds 1024 bytes.",
      "展開した別名が不正、または 1024 バイトを超えています。",
    );
  }
  return null;
}

/** Validate the actual content being saved, including direct raw-editor changes. */
export function modelAliasSaveError(content: string, t: LocaleText): string | null {
  try {
    const parsed = JSON.parse(content);
    return modelAliasValidationMessage(parseModelAliases(parsed?.metadata?.localProxyModelAliases), t);
  } catch {
    // Preserve existing native/non-JSON profile handling.
    return null;
  }
}
