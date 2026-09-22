import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import ProfilesListView from "./ListView";

vi.mock("../../components/UniversalProviderManager", () => ({
  default: () => <div>Shared provider controls</div>,
}));

const profile = {
  id: "provider-1",
  name: "Primary API",
  tool_id: "claude",
  config_snapshot: JSON.stringify({ env: { ANTHROPIC_BASE_URL: "https://api.example.com" } }),
  sort_order: 0,
  created_at: "2026-01-01T00:00:00",
  updated_at: null,
};

function createProps(): ComponentProps<typeof ProfilesListView> {
  return {
    locale: "zh",
    localeText: (zh) => zh,
    profiles: [profile],
    activeIds: [],
    tools: [{ id: "claude", name: "Claude", installed: true }],
    installedTools: [{ id: "claude", name: "Claude", installed: true }],
    toolCounts: { claude: 1 },
    filterTool: "claude",
    filteredProfiles: [profile],
    activeIdSet: new Set(),
    pingResults: {},
    streamCheckResults: {},
    sharedGroupCounts: {},
    search: "",
    searchInputRef: { current: null },
    reorderEnabled: false,
    draggingProfileId: null,
    dragOverProfileId: null,
    pingingId: null,
    streamCheckingId: null,
    batchStreamChecking: false,
    applying: null,
    profileCardText: { applyButton: "启用", activeButton: "已启用" },
    handleRefreshProfiles: vi.fn(),
    handleOpenCreateProfile: vi.fn(),
    handleSearchChange: vi.fn(),
    handleClearSearch: vi.fn(),
    handleToggleFilterTool: vi.fn(),
    handleCardDragStart: vi.fn(),
    handleCardDragEnter: vi.fn(),
    handleCardDragEnd: vi.fn(),
    handleCardDrop: vi.fn(),
    handlePing: vi.fn(),
    handleStreamCheck: vi.fn(),
    handleUsage: vi.fn(),
    handleStreamCheckAll: vi.fn(),
    doApply: vi.fn(),
    handleDuplicate: vi.fn(),
    openEditModal: vi.fn(),
    handleDelete: vi.fn(),
  };
}

describe("ProfilesListView", () => {
  it("keeps switching prominent and applies the selected profile", () => {
    const props = createProps();
    render(<ProfilesListView {...props} />);

    expect(screen.getByText("Primary API")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "启用" }));
    expect(props.doApply).toHaveBeenCalledWith(profile);
    fireEvent.click(screen.getByRole("button", { name: "新增" }));
    expect(props.handleOpenCreateProfile).toHaveBeenCalledTimes(1);
  });

  it("reveals shared providers only when requested", () => {
    render(<ProfilesListView {...createProps()} />);
    const toggle = screen.getByRole("button", { name: "跨工具共享配置" });

    expect(screen.queryByText("Shared provider controls")).toBeNull();
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("Shared provider controls")).toBeTruthy();
  });

  it("renders every tool in the wrapping switcher and changes the active filter", () => {
    const props = createProps();
    props.tools = [
      { id: "claude", name: "Claude", installed: true },
      { id: "codex", name: "Codex", installed: true },
      { id: "gemini", name: "Gemini", installed: false },
      { id: "grok", name: "Grok", installed: false },
      { id: "opencode", name: "OpenCode", installed: false },
      { id: "openclaw", name: "OpenClaw", installed: false },
      { id: "hermes", name: "Hermes", installed: false },
      { id: "pi", name: "Pi", installed: false },
    ];
    props.toolCounts = { claude: 1, codex: 2 };

    render(<ProfilesListView {...props} />);

    const switcher = screen.getByRole("tablist", { name: "工具" });
    expect(switcher.children).toHaveLength(8);
    expect(screen.getByRole("tab", { name: "Claude (1)" }).getAttribute("aria-selected")).toBe("true");

    fireEvent.click(screen.getByRole("tab", { name: "Codex (2)" }));
    expect(props.handleToggleFilterTool).toHaveBeenCalledWith("codex");
  });
});
