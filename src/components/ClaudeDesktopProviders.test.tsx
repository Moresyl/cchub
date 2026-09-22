import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ClaudeDesktopProviders from "./ClaudeDesktopProviders";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("./Toast", () => ({ showToast: vi.fn() }));

const state = {
  providers: [
    {
      id: "one",
      name: "Gateway",
      baseUrl: "https://gateway.example.com",
      hasApiKey: true,
      models: ["claude-sonnet-4-6"],
    },
  ],
  activeId: "one",
  profilePath: "C:/Claude-3p/configLibrary/cchub.json",
};

describe("Claude Desktop direct providers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(state);
  });

  it("does not expose a stored secret and retains it when editing", async () => {
    render(<ClaudeDesktopProviders locale="en" />);
    fireEvent.click(await screen.findByRole("button", { name: "Edit Gateway" }));
    expect(screen.getByLabelText("API Key")).toHaveProperty("value", "");
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "New name" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "save_claude_desktop_provider",
        expect.objectContaining({
          id: "one",
          name: "New name",
          apiKey: "",
          models: ["claude-sonnet-4-6"],
        }),
      ),
    );
  });

  it("rejects insecure remote gateways before saving", async () => {
    render(<ClaudeDesktopProviders locale="en" />);
    fireEvent.click(await screen.findByRole("button", { name: "New provider" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "New" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Gateway URL" }), {
      target: { value: "http://remote.example.com" },
    });
    fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", "Use HTTPS or a loopback HTTP gateway");
    expect(invokeMock.mock.calls.some(([command]) => command === "save_claude_desktop_provider")).toBe(false);
  });

  it("restores official mode only after confirmation", async () => {
    render(<ClaudeDesktopProviders locale="en" />);
    fireEvent.click(await screen.findByRole("button", { name: "Restore official" }));
    expect(invokeMock.mock.calls.some(([command]) => command === "restore_claude_desktop_official")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("restore_claude_desktop_official", undefined));
  });
});
