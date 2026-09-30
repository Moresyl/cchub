import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createDefaultStructuredFields } from "../../lib/configProfiles";
import { ProfileModelsSection } from "./ModelsSection";

describe("profile model section", () => {
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
