import { create } from "zustand";
import { persist } from "zustand/middleware";

const MAX_RECENT_COMMANDS = 6;

interface CommandPaletteState {
  recentCommandIds: string[];
  recordCommand: (id: string) => void;
}

export const useCommandPaletteStore = create<CommandPaletteState>()(
  persist(
    (set) => ({
      recentCommandIds: [],
      recordCommand: (id) => {
        set((state) => ({
          recentCommandIds: [id, ...state.recentCommandIds.filter((currentId) => currentId !== id)].slice(
            0,
            MAX_RECENT_COMMANDS,
          ),
        }));
      },
    }),
    {
      name: "cchub-command-palette",
    },
  ),
);
