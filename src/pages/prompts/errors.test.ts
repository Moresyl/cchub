import { describe, expect, it } from "vitest";
import { promptErrorText, READBACK_ERROR } from "./errors";
const zh = (zhText: string) => zhText;

describe("prompt error feedback", () => {
  it.each([
    [READBACK_ERROR, "修改已保存"],
    ["Prompt library changed; reload before saving", "已被外部修改"],
    ["Error: Prompt file changed externally; reload", "已被外部修改"],
    ["Prompt file exceeds the 1 MiB limit", "超过 1 MiB"],
    ["Prompt content is too large", "超过 1 MiB"],
    ["Prompt file must contain valid UTF-8", "UTF-8"],
    ["Prompt target must be a regular file", "不是普通文件"],
    ["Cannot read prompt file", "检查访问权限"],
    ["Cannot inspect prompt file", "检查访问权限"],
    ["Prompt description is too long", "超过 2000"],
    ["Prompt name must contain 1 to 120 characters", "最多 120"],
  ])("describes the cause and next action for %s", (failure, expected) => {
    expect(promptErrorText(failure, zh)).toContain(expected);
  });
  it("does not echo unknown internal failures", () => {
    const message = promptErrorText('SQL failed: private="PRIVATE_VALUE"', zh);
    expect(message).toContain("重新加载最新状态");
    expect(message).not.toContain("PRIVATE_VALUE");
  });
});
