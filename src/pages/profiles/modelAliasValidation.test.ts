import { describe, expect, it } from "vitest";
import { modelAliasSaveError, modelAliasValidationMessage } from "./modelAliasValidation";
const zh = (text: string) => text;
const en = (_: string, text: string) => text;

describe("model alias save validation", () => {
  it("checks the current raw snapshot and permits an explicitly repaired snapshot", () => {
    const snapshot = (value: unknown) => JSON.stringify({ metadata: { localProxyModelAliases: value } });
    expect(modelAliasSaveError(snapshot([{ model: "core", upstream: "" }]), zh)).toContain("第 1 条");
    expect(modelAliasSaveError(snapshot(null), en)).toContain("malformed");
    expect(modelAliasSaveError(snapshot([{ model: "core", upstream: "vendor/*" }]), zh)).toBeNull();
    expect(modelAliasSaveError(snapshot([]), zh)).toBeNull();
    expect(modelAliasSaveError("model = 'native'", zh)).toBeNull();
    expect(modelAliasSaveError("{}", zh)).toBeNull();
  });
  it("reports expansion overflow for the preview without discarding the rules", () => {
    expect(
      modelAliasValidationMessage({ localProxyModelAliases: [{ model: "*", upstream: "*".repeat(1024) }] }, zh, "long"),
    ).toContain("展开后的别名");
  });
});
