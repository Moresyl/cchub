import type { Text } from "./types";

export const READBACK_ERROR =
  "The change was saved, but its current state could not be read. Reload before continuing.";

export function promptErrorText(error: string, text: Text): string {
  const message = error.replace(/^Error:\s*/, "");
  if (message === READBACK_ERROR) {
    return text(
      "修改已保存，但暂时无法读取最新状态。请重新加载后再继续。",
      "The change was saved. Reload to read its current state before continuing.",
      "変更は保存されましたが、最新の状態を読み取れません。再読み込みしてから続けてください。",
    );
  }
  if (message.startsWith("Prompt library changed") || message.startsWith("Prompt file changed externally")) {
    return text(
      "指令库或当前文件已被外部修改。草稿已保留，请重新加载并核对后再保存。",
      "The library or live file changed externally. Your draft is retained; reload and review before saving.",
      "指示ライブラリまたは現在のファイルが外部で変更されました。下書きは保持されています。再読み込みして確認してください。",
    );
  }
  if (message.startsWith("Prompt file exceeds") || message === "Prompt content is too large") {
    return text(
      "当前文件或指令内容超过 1 MiB，请缩短后再试。",
      "The file or instructions exceed 1 MiB. Shorten them before retrying.",
      "ファイルまたは指示が 1 MiB を超えています。短くしてから再試行してください。",
    );
  }
  if (message === "Prompt file must contain valid UTF-8") {
    return text(
      "指令文件不是有效的 UTF-8 文本。请检查编码后重新加载。",
      "The instruction file is not valid UTF-8. Check its encoding and reload.",
      "指示ファイルが有効な UTF-8 ではありません。文字コードを確認して再読み込みしてください。",
    );
  }
  if (message === "Prompt target must be a regular file") {
    return text(
      "指令路径不是普通文件。请检查文件路径后重新加载。",
      "The instruction path is not a regular file. Check the path and reload.",
      "指示のパスが通常のファイルではありません。パスを確認して再読み込みしてください。",
    );
  }
  if (message === "Cannot read prompt file" || message === "Cannot inspect prompt file") {
    return text(
      "无法读取当前指令文件。请检查访问权限后重新加载。",
      "Cannot read the live instruction file. Check access permissions and reload.",
      "現在の指示ファイルを読み取れません。アクセス権を確認して再読み込みしてください。",
    );
  }
  if (message === "Prompt description is too long") {
    return text(
      "说明超过 2000 个字符，请缩短后再保存。",
      "Description exceeds 2000 characters. Shorten it before saving.",
      "説明が 2000 文字を超えています。短くしてから保存してください。",
    );
  }
  if (message.startsWith("Prompt name must contain")) {
    return text(
      "名称必填，最多 120 个字符。",
      "A name is required, up to 120 characters.",
      "名前は必須で、120 文字以内です。",
    );
  }
  return text(
    "操作未完成，请重新加载最新状态后再试。",
    "The operation could not be completed. Reload the latest state and retry.",
    "操作を完了できませんでした。最新の状態を再読み込みして再試行してください。",
  );
}
