import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createDefaultStructuredFields } from "../../lib/configProfiles";
import { ProfileModelsSection } from "./ModelsSection";

describe("profile model section", () => {
  it("shows a discovered default and preserves the saved effort after refresh", () => {
    const onDraftChange = vi.fn();
    const props = {
      locale: "zh",
      localeText: (zh: string) => zh,
      draftTool: "codex",
      draftFields: { ...createDefaultStructuredFields("codex"), model: "custom", codexReasoningEffort: "high" },
      fetchedModels: ["custom"],
      fetchedModelDetails: [{ id: "custom", supportedReasoningLevels: ["high", "max"], defaultReasoningEffort: "max" }],
      fetchingModels: false,
      modelFetchError: null,
      onFetchModels: vi.fn(),
      onDraftChange,
    };
    const view = render(<ProfileModelsSection {...props} />);
    expect(screen.getByText("默认推理等级")).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "推理强度" }).textContent).toBe("high");
    view.rerender(<ProfileModelsSection {...props} fetchedModelDetails={[]} />);
    expect(screen.getByRole("combobox", { name: "推理强度" }).textContent).toBe("high");
    expect(onDraftChange).not.toHaveBeenCalled();
  });
  it("binds reasoning to the selected model and preserves the original effort through capability changes", () => {
    const fields = { ...createDefaultStructuredFields("codex"), model: "plain", codexReasoningEffort: "" };
    const onDraftChange = vi.fn();
    const props = {
      locale: "zh",
      localeText: (zh: string) => zh,
      draftTool: "codex",
      draftFields: fields,
      fetchedModels: ["plain", "thinking"],
      fetchedModelDetails: [
        { id: "plain", supportedReasoningLevels: [] },
        { id: "thinking", supportedReasoningLevels: ["max"] },
      ],
      fetchingModels: false,
      modelFetchError: null,
      onFetchModels: vi.fn(),
      onDraftChange,
    };
    const view = render(<ProfileModelsSection {...props} />);
    expect(screen.getByRole("combobox", { name: "推理强度" })).toHaveProperty("disabled", true);
    const label = [...view.container.querySelectorAll("label")].find((element) => element.textContent === "推理强度");
    expect(label?.control).toBe(screen.getByRole("combobox", { name: "推理强度" }));
    expect(screen.getByText("无可配置等级")).toBeTruthy();
    view.rerender(
      <ProfileModelsSection {...props} draftFields={{ ...fields, model: "thinking", codexReasoningEffort: "high" }} />,
    );
    expect(screen.getByRole("combobox", { name: "推理强度" }).textContent).toContain("high（原配置");
    expect(screen.getByText("max")).toBeTruthy();
    view.rerender(
      <ProfileModelsSection
        {...props}
        draftFields={{ ...fields, model: "custom", codexReasoningEffort: "custom-effort" }}
      />,
    );
    expect(screen.getByRole("combobox", { name: "推理强度" }).textContent).toBe("custom-effort");
    expect(screen.getByText(/尚未获取当前模型/)).toBeTruthy();
    expect(onDraftChange).not.toHaveBeenCalled();
  });

  it("shows reported capabilities and applies them only after the explicit action", () => {
    const onChange = vi.fn();
    render(
      <ProfileModelsSection
        locale="zh"
        localeText={(zh) => zh}
        draftTool="opencode"
        draftFields={{ ...createDefaultStructuredFields("opencode"), model: "a", openCodeContextLimit: "10000" }}
        fetchedModels={["a"]}
        fetchedModelDetails={[
          {
            id: "a",
            contextWindow: 200000,
            maxOutputTokens: 16000,
            nativeEndpoints: ["/messages"],
            supportedReasoningLevels: ["low", "high"],
            inputModalities: ["text", "image"],
          },
        ]}
        fetchingModels={false}
        modelFetchError={null}
        onFetchModels={vi.fn()}
        onDraftChange={onChange}
      />,
    );
    expect(screen.getByText("200,000 tokens")).toBeTruthy();
    expect(screen.getByText("low · high")).toBeTruthy();
    expect(screen.getByLabelText("上下文上限")).toHaveProperty("value", "10000");
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "填入当前模型" }));
    expect(onChange).toHaveBeenCalledWith("opencode", {
      openCodeContextLimit: "200000",
      openCodeOutputLimit: "16000",
      openCodeInputModalities: "text,image",
    });
  });

  it("keeps the fallback input mounted while unrelated catalog state changes", () => {
    const fields = { ...createDefaultStructuredFields("claude"), model: "custom" };
    const props = {
      locale: "zh",
      localeText: (zh: string) => zh,
      draftTool: "claude",
      draftFields: fields,
      fetchedModels: [],
      fetchedModelDetails: [],
      fetchingModels: false,
      modelFetchError: null,
      onFetchModels: vi.fn(),
      onDraftChange: vi.fn(),
    };
    const view = render(<ProfileModelsSection {...props} />);
    const input = screen.getByLabelText("主模型");
    input.focus();
    view.rerender(<ProfileModelsSection {...props} fetchingModels />);
    expect(screen.getByLabelText("主模型")).toBe(input);
    expect(document.activeElement).toBe(input);
  });
});
