import { useState } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProfileEndpointProbePanel from "./ProfileEndpointProbePanel";
import { collectProbeEndpoints, type EndpointLatency } from "./profileEndpointProbe";
import { buildStructuredConfig, createDefaultStructuredFields, parseStructuredConfig } from "../lib/configProfiles";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const defaults = {
  locale: "zh",
  localeText: (zh: string) => zh,
  appId: "claude",
  providerId: "saved",
  baseUrl: "https://primary.test",
  candidates: "",
  customEndpoints: [],
  onCustomEndpointsChange: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("draft endpoint probe", () => {
  it("stages additions and removals in the draft and persists them only through the normal builder", () => {
    let current: string[] = [];
    function Editor() {
      const [urls, setUrls] = useState<string[]>([]);
      current = urls;
      return <ProfileEndpointProbePanel {...defaults} customEndpoints={urls} onCustomEndpointsChange={setUrls} />;
    }
    const view = render(<Editor />);
    const input = screen.getByLabelText("自定义端点");
    fireEvent.change(input, { target: { value: " https://draft.test/ " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(current).toEqual(["https://draft.test"]);
    expect(invoke).not.toHaveBeenCalled();
    const content = buildStructuredConfig("claude", {
      ...createDefaultStructuredFields("claude"),
      customEndpoints: current,
    });
    expect(parseStructuredConfig("claude", content).customEndpoints).toEqual(current);
    fireEvent.click(screen.getByRole("button", { name: "删除端点 https://draft.test" }));
    expect(current).toEqual([]);
    expect(invoke).not.toHaveBeenCalled();
    view.unmount();
  });

  it("validates custom addresses and associates localized errors with the shared input", () => {
    render(<ProfileEndpointProbePanel {...defaults} customEndpoints={["https://known.test"]} />);
    const input = screen.getByLabelText("自定义端点");
    expect(input.getAttribute("data-slot")).toBe("input");
    for (const value of ["ftp://invalid.test", "https://user:secret@invalid.test", "incomplete"]) {
      fireEvent.change(input, { target: { value } });
      fireEvent.click(screen.getByRole("button", { name: "添加" }));
      expect(screen.getByRole("alert").textContent).toContain("HTTP(S)");
      expect(input.getAttribute("aria-describedby")).toBe(screen.getByRole("alert").id);
    }
    fireEvent.change(input, { target: { value: "https://known.test/" } });
    fireEvent.click(screen.getByRole("button", { name: "添加" }));
    expect(screen.getByRole("alert").textContent).toContain("已经添加");
    expect(defaults.onCustomEndpointsChange).not.toHaveBeenCalled();
  });

  it("rejects more than 128 custom endpoints", () => {
    render(
      <ProfileEndpointProbePanel
        {...defaults}
        customEndpoints={Array.from({ length: 128 }, (_, i) => `https://${i}.test`)}
      />,
    );
    fireEvent.change(screen.getByLabelText("自定义端点"), { target: { value: "https://new.test" } });
    fireEvent.click(screen.getByRole("button", { name: "添加" }));
    expect(screen.getByRole("alert").textContent).toContain("128");
    expect(defaults.onCustomEndpointsChange).not.toHaveBeenCalled();
  });

  it("clears endpoint input and errors when switching profiles", () => {
    const view = render(<ProfileEndpointProbePanel {...defaults} />);
    fireEvent.change(screen.getByLabelText("自定义端点"), { target: { value: "bad" } });
    fireEvent.click(screen.getByRole("button", { name: "添加" }));
    view.rerender(<ProfileEndpointProbePanel {...defaults} providerId="other" />);
    expect(screen.getByLabelText("自定义端点")).toHaveProperty("value", "");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("disables mutation controls when no draft change handler is provided", () => {
    render(
      <ProfileEndpointProbePanel
        {...defaults}
        onCustomEndpointsChange={undefined}
        customEndpoints={["https://known.test"]}
      />,
    );
    expect(screen.getByLabelText("自定义端点")).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "删除端点 https://known.test" })).toHaveProperty("disabled", true);
  });

  it("uses draft URLs, prevents duplicate requests and distinguishes HTTP errors from model success", async () => {
    const response = deferred<EndpointLatency[]>();
    invoke.mockReturnValue(response.promise);
    render(
      <ProfileEndpointProbePanel
        {...defaults}
        candidates="https://other.test/"
        customEndpoints={["https://custom.test"]}
      />,
    );
    const button = screen.getByRole("button", { name: "开始测速" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("test_api_endpoints", {
      urls: ["https://primary.test", "https://other.test", "https://custom.test"],
      timeoutSecs: 10,
    });
    await act(async () =>
      response.resolve([
        { url: "https://primary.test", latency: 20, status: 401, error: null },
        { url: "https://other.test", latency: 10, status: 200, error: null },
        { url: "https://custom.test", latency: null, status: null, error: "Request timed out" },
      ]),
    );
    expect(screen.getByText("HTTP 401 · 20 ms")).toBeTruthy();
    expect(screen.getByText("Request timed out")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("密钥和模型尚未验证");
    const rows = [...screen.getByRole("status").parentElement!.querySelectorAll("li")];
    expect(rows[0].textContent).toContain("other.test");
    expect(rows[1].querySelector("svg")?.getAttribute("style")).toContain("var(--warning)");
  });

  it.each(["baseUrl", "providerId", "appId", "customEndpoints"])(
    "discards stale successes after %s changes",
    async (field) => {
      const old = deferred<EndpointLatency[]>();
      const next = deferred<EndpointLatency[]>();
      invoke.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
      const view = render(<ProfileEndpointProbePanel {...defaults} />);
      fireEvent.click(screen.getByRole("button", { name: "开始测速" }));
      const changes =
        field === "customEndpoints"
          ? { customEndpoints: ["https://new.test"] }
          : { [field]: field === "baseUrl" ? "https://new.test" : "other" };
      view.rerender(<ProfileEndpointProbePanel {...defaults} {...changes} />);
      fireEvent.click(screen.getByRole("button", { name: "开始测速" }));
      await act(async () => old.resolve([{ url: "https://stale.test", latency: 1, status: 200, error: null }]));
      expect(screen.queryByText("https://stale.test")).toBeNull();
      expect(screen.getByRole("button", { name: "测速中…" })).toHaveProperty("disabled", true);
      await act(async () => next.resolve([{ url: "https://current.test", latency: 2, status: 200, error: null }]));
      expect(screen.getByText("https://current.test")).toBeTruthy();
      expect(screen.getByRole("button", { name: "开始测速" })).toHaveProperty("disabled", false);
    },
  );

  it("ignores stale errors, reports current errors and can retry", async () => {
    const old = deferred<EndpointLatency[]>();
    invoke
      .mockReturnValueOnce(old.promise)
      .mockRejectedValueOnce(new Error("current failure"))
      .mockResolvedValueOnce([]);
    const view = render(<ProfileEndpointProbePanel {...defaults} />);
    fireEvent.click(screen.getByRole("button", { name: "开始测速" }));
    view.rerender(<ProfileEndpointProbePanel {...defaults} baseUrl="https://new.test" />);
    await act(async () => old.reject(new Error("stale failure")));
    expect(screen.queryByRole("alert")).toBeNull();
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "开始测速" })));
    expect(screen.getByRole("alert").textContent).toContain("current failure");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "开始测速" })));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("does not run for incomplete URLs", () => {
    render(<ProfileEndpointProbePanel {...defaults} baseUrl="https://" />);
    expect(screen.getByRole("button", { name: "开始测速" })).toHaveProperty("disabled", true);
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("endpoint collection", () => {
  it("normalizes, deduplicates, excludes invalid credentials and enforces the probe limit", () => {
    expect(
      collectProbeEndpoints(" https://a.test/// ", "https://a.test,\nftp://x.test\nhttps://u:p@x.test\nincomplete", [
        "https://b.test/",
      ]),
    ).toEqual(["https://a.test", "https://b.test"]);
    expect(
      collectProbeEndpoints(
        "",
        "",
        Array.from({ length: 70 }, (_, i) => `https://${i}.test`),
      ),
    ).toHaveLength(64);
  });
});
