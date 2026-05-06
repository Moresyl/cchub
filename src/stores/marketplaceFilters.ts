import { create } from "zustand";
import { persist } from "zustand/middleware";

export type MarketTab = "mcp" | "skills";
export type McpCategory =
  | "all"
  | "installed"
  | "search"
  | "database"
  | "ai"
  | "dev-tools"
  | "browser"
  | "filesystem"
  | "cloud"
  | "productivity";
export type SkillCategory =
  | "all"
  | "installed"
  | "development"
  | "testing"
  | "documentation"
  | "devops"
  | "ai-ml"
  | "security"
  | "backend";

type ActiveToolUpdater = string | ((current: string) => string);

interface MarketplaceFiltersState {
  tab: MarketTab;
  search: string;
  mcpCategory: McpCategory;
  skillCategory: SkillCategory;
  activeTool: string;
  setTab: (tab: MarketTab) => void;
  setSearch: (search: string) => void;
  setMcpCategory: (category: McpCategory) => void;
  setSkillCategory: (category: SkillCategory) => void;
  setActiveTool: (updater: ActiveToolUpdater) => void;
}

export const useMarketplaceFilters = create<MarketplaceFiltersState>()(
  persist(
    (set) => ({
      tab: "mcp",
      search: "",
      mcpCategory: "all",
      skillCategory: "all",
      activeTool: "claude",
      setTab: (tab) => set({ tab }),
      setSearch: (search) => set({ search }),
      setMcpCategory: (mcpCategory) => set({ mcpCategory }),
      setSkillCategory: (skillCategory) => set({ skillCategory }),
      setActiveTool: (updater) => {
        set((state) => ({
          activeTool: typeof updater === "function" ? updater(state.activeTool) : updater,
        }));
      },
    }),
    {
      name: "cchub-marketplace-filters",
    },
  ),
);
