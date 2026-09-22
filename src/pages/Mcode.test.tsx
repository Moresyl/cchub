import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Mcode from "./Mcode";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("../components/Toast", () => ({ showToast: vi.fn() }));

const state = {
  configPath: "C:/Users/test/.minimax/config.yaml",
  installed: true,
  providers: {
    sample: {
      kind: "custom",
      enabled: true,
      api: "anthropic-messages",
      options: { baseURL: "https://old.example", apiKey: "secret", headers: { source: "test" } },
      models: { model: { name: "Display", limit: { context: 1000 } } },
    },
  },
};

describe("MiniMax Code page", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => (command === "get_mcode_state" ? state : null));
  });

  it("edits one provider while preserving unexposed configuration", async () => {
    render(<Mcode />);
    fireEvent.click(await screen.findByRole("button", { name: /编辑 sample|Edit sample/ }));
    fireEvent.change(screen.getByRole("textbox", { name: /端点 URL|Endpoint URL/ }), {
      target: { value: "https://new.example" },
    });
    fireEvent.click(screen.getByRole("button", { name: /保存|Save/ }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("save_mcode_provider", expect.objectContaining({ id: "sample" })),
    );
    const saved = invokeMock.mock.calls.find(([command]) => command === "save_mcode_provider")?.[1].provider;
    expect(saved.options.headers).toEqual({ source: "test" });
    expect(saved.options.baseURL).toBe("https://new.example");
    expect(saved.models.model.limit.context).toBe(1000);
  });

  it("does not submit invalid endpoints", async () => {
    render(<Mcode />);
    fireEvent.click(await screen.findByRole("button", { name: /编辑 sample|Edit sample/ }));
    fireEvent.change(screen.getByRole("textbox", { name: /端点 URL|Endpoint URL/ }), {
      target: { value: "file:///private" },
    });
    fireEvent.click(screen.getByRole("button", { name: /保存|Save/ }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(invokeMock.mock.calls.some(([command]) => command === "save_mcode_provider")).toBe(false);
  });
});
