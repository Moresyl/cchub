import { create } from "zustand";
import { persist } from "zustand/middleware";

const LEGACY_STORAGE_KEY = "cchub-autopilot-form";

export interface AutopilotFormState {
  taskFiles: string[];
  workdir: string;
  model: string;
  profile: string;
  interval: string;
  maxAttempts: string;
  fresh: boolean;
  dryRun: boolean;
  skipGitCheck: boolean;
  bypass: boolean;
  fullAuto: boolean;
  verbose: boolean;
}

export const DEFAULT_AUTOPILOT_FORM: AutopilotFormState = {
  taskFiles: [],
  workdir: "",
  model: "",
  profile: "",
  interval: "3",
  maxAttempts: "0",
  fresh: false,
  dryRun: false,
  skipGitCheck: false,
  bypass: true,
  fullAuto: true,
  verbose: false,
};

type FormUpdater = AutopilotFormState | ((current: AutopilotFormState) => AutopilotFormState);

interface AutopilotFormStore {
  form: AutopilotFormState;
  setForm: (updater: FormUpdater) => void;
  resetForm: () => void;
}

function loadLegacyForm(): AutopilotFormState {
  if (typeof localStorage === "undefined") return DEFAULT_AUTOPILOT_FORM;
  try {
    const raw = localStorage.getItem(LEGACY_STORAGE_KEY);
    if (!raw) return DEFAULT_AUTOPILOT_FORM;
    const parsed = JSON.parse(raw) as Partial<AutopilotFormState> & {
      taskFile?: string;
      codexBin?: string;
    };
    const taskFiles = Array.isArray(parsed.taskFiles)
      ? parsed.taskFiles
      : typeof parsed.taskFile === "string" && parsed.taskFile.trim()
        ? [parsed.taskFile]
        : [];
    const { taskFile: _legacyTaskFile, codexBin: _legacyCodexBin, ...rest } = parsed;
    return { ...DEFAULT_AUTOPILOT_FORM, ...rest, taskFiles };
  } catch (error) {
    console.warn("Failed to load legacy Autopilot form", error);
    return DEFAULT_AUTOPILOT_FORM;
  }
}

export const useAutopilotFormStore = create<AutopilotFormStore>()(
  persist(
    (set) => ({
      form: loadLegacyForm(),
      setForm: (updater) => {
        set((state) => ({
          form: typeof updater === "function" ? updater(state.form) : updater,
        }));
      },
      resetForm: () => set({ form: DEFAULT_AUTOPILOT_FORM }),
    }),
    {
      name: "cchub-autopilot-form-store",
      partialize: (state) => ({ form: state.form }),
    },
  ),
);
