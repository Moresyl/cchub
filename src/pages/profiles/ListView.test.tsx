import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { MemoryRouter } from "react-router-dom";
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
    profileCardText: {
      activeTag: "当前生效",
      pingFast: "快速",
      pingMedium: "一般",
      pingSlow: "较慢",
      pingError: "异常",
      streamHealthy: "流检通过",
      streamReachable: "流检可达",
      streamUnsupported: "流检暂不支持",
      streamUnconfigured: "流检未配置",
      streamError: "流检异常",
      dragEnabledTitle: "拖拽调整顺序",
      dragDisabledTitle: "当前不可排序",
      pingTitle: "端点测速",
      streamTitle: "流式健康检查",
      usageTitle: "查询用量",
      duplicateTitle: "复制",
      editTitle: "编辑",
      deleteTitle: "删除",
      moreTitle: "更多操作",
      applyButton: "启用",
      activeButton: "已启用",
    },
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

function renderListView(props = createProps()) {
  return render(
    <MemoryRouter>
      <ProfilesListView {...props} />
    </MemoryRouter>,
  );
}

describe("ProfilesListView", () => {
  it("keeps switching prominent and applies the selected profile", () => {
    const props = createProps();
    renderListView(props);

    expect(screen.getByText("Primary API")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "启用" }));
    expect(props.doApply).toHaveBeenCalledWith(profile);
    fireEvent.click(screen.getByRole("button", { name: "新增配置" }));
    expect(props.handleOpenCreateProfile).toHaveBeenCalledTimes(1);
  });

  it("reveals shared providers only when requested", () => {
    renderListView();
    const toggle = screen.getByRole("button", { name: "共享配置" });

    expect(screen.queryByText("Shared provider controls")).toBeNull();
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByText("Shared provider controls")).toBeTruthy();
  });

  it("keeps secondary profile actions in a focused menu", () => {
    const props = createProps();
    renderListView(props);

    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "更多操作" }));
    expect(screen.getByRole("menu")).toBeTruthy();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("menu")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "更多操作" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "复制" }));
    expect(props.handleDuplicate).toHaveBeenCalledWith(profile);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("shows available tools in the compact switcher and changes the active filter", () => {
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

    renderListView(props);

    const switcher = screen.getByRole("tablist", { name: "工具" });
    expect(switcher.children).toHaveLength(2);
    expect(screen.queryByRole("tab", { name: "Gemini (0)" })).toBeNull();
    expect(screen.getByRole("tab", { name: "Claude (1)" }).getAttribute("aria-selected")).toBe("true");

    fireEvent.click(screen.getByRole("tab", { name: "Codex (2)" }));
    expect(props.handleToggleFilterTool).toHaveBeenCalledWith("codex");
  });
});
