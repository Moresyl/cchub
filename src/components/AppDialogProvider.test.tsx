import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { AppDialogProvider, useAppDialog } from "./AppDialogProvider";

function DialogHarness() {
  const appDialog = useAppDialog();
  const [result, setResult] = useState("pending");
  return (
    <>
      <button
        onClick={() =>
          void appDialog
            .confirm({ title: "Run check", message: "This may use quota", confirmText: "Run" })
            .then((value) => setResult(String(value)))
        }
      >
        Open confirm
      </button>
      <button
        onClick={() =>
          void appDialog
            .prompt({ title: "Rename", message: "Enter a name", defaultValue: "Backup" })
            .then((value) => setResult(value ?? "cancelled"))
        }
      >
        Open prompt
      </button>
      <output>{result}</output>
    </>
  );
}

describe("AppDialogProvider", () => {
  it("resolves a styled confirmation", async () => {
    render(
      <AppDialogProvider>
        <DialogHarness />
      </AppDialogProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open confirm" }));
    expect(screen.getByRole("dialog", { name: "Run check" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Run" }));
    await waitFor(() => expect(screen.getByText("true")).toBeTruthy());
  });

  it("returns edited prompt input and supports cancellation", async () => {
    render(
      <AppDialogProvider>
        <DialogHarness />
      </AppDialogProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open prompt" }));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "Daily backup" } });
    fireEvent.click(screen.getByRole("button", { name: "确认" }));
    await waitFor(() => expect(screen.getByText("Daily backup")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: "Open prompt" }));
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.getByText("cancelled")).toBeTruthy());
  });
});
