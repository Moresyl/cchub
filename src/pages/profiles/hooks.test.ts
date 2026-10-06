import { describe, expect, it, vi } from "vitest";
import { performOpenEditModal, type OpenEditModalContext } from "./hooks";
import type { ConfigProfile } from "./helpers";

function context(profile: ConfigProfile, profiles = [profile]): OpenEditModalContext {
  return {
    profile,
    profiles,
    setEditingProfile: vi.fn(),
    setShowCreateModal: vi.fn(),
    setDraftName: vi.fn(),
    setDraftTool: vi.fn(),
    setDraftTargetTools: vi.fn(),
    setDraftContent: vi.fn(),
    setShowApiKey: vi.fn(),
    setFetchingModels: vi.fn(),
    setFetchedModels: vi.fn(),
    setFetchedModelDetails: vi.fn(),
    setModelFetchError: vi.fn(),
    setDraftFields: vi.fn(),
    setDraftLoading: vi.fn(),
    resetStructuredDraft: vi.fn(),
    setNativeOpenCode: vi.fn(),
  };
}

function profile(snapshot: unknown, tool = "opencode"): ConfigProfile {
  return {
    id: "current",
    name: "Local",
    tool_id: tool,
    config_snapshot: JSON.stringify(snapshot),
    sort_order: 0,
    created_at: null,
    updated_at: null,
  };
}

describe("profile editor opening", () => {
  it("preserves native settings, variants and extensions rather than rebuilding with a legacy SDK", () => {
    const snapshot = {
      settings: { apiKey: "fixture" },
      metadata: { nativeFormat: "providers", nativeProviderId: "local", nativeModelId: "m#fast" },
      models: { m: { variants: [{ id: "fast", settings: { effort: "low" } }], extension: { keep: true } } },
    };
    const ctx = context(profile(snapshot));
    performOpenEditModal(ctx);
    expect(ctx.setNativeOpenCode).toHaveBeenCalledWith(true);
    expect(ctx.setDraftContent).toHaveBeenCalledTimes(1);
    expect(JSON.parse(vi.mocked(ctx.setDraftContent).mock.calls[0][0])).toEqual(snapshot);
    expect(ctx.resetStructuredDraft).not.toHaveBeenCalled();
    expect(ctx.setDraftLoading).toHaveBeenCalledWith(false);
  });

  it("preserves raw drafts for tools without structured forms", () => {
    const snapshot = { arbitrary: { retained: true } };
    const ctx = context(profile(snapshot, "custom-tool"));
    performOpenEditModal(ctx);
    expect(ctx.setDraftContent).toHaveBeenCalledTimes(1);
    expect(JSON.parse(vi.mocked(ctx.setDraftContent).mock.calls[0][0])).toEqual(snapshot);
    expect(ctx.resetStructuredDraft).not.toHaveBeenCalled();
    expect(ctx.setNativeOpenCode).toHaveBeenCalledWith(false);
  });

  it("does not fold an unrelated native shared profile into the legacy form", () => {
    const current = { ...profile({ options: { apiKey: "current" } }), source_type: "shared", source_key: "group" };
    const other = {
      ...profile({ package: "native-package", settings: { apiKey: "unrelated" } }),
      id: "other",
      source_type: "shared",
      source_key: "group",
    };
    const ctx = context(current, [current, other]);
    performOpenEditModal(ctx);
    const saved = JSON.parse(vi.mocked(ctx.setDraftContent).mock.lastCall![0]);
    expect(saved.options.apiKey).toBe("current");
    expect(saved.settings).toBeUndefined();
    expect(saved.npm).not.toBe("native-package");
    expect(ctx.setNativeOpenCode).toHaveBeenCalledWith(false);
  });
});
