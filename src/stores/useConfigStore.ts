import { create } from "zustand";
import { AppConfig, getConfig, setConfig } from "../lib/ipc";

interface ConfigStore {
  config: AppConfig | null;
  draft: AppConfig | null;
  error: string | null;
  isSaving: boolean;

  load: () => Promise<void>;
  update: (partial: Partial<AppConfig>) => void;
  save: () => Promise<void>;
}

export const useConfigStore = create<ConfigStore>((set, get) => ({
  config: null,
  draft: null,
  error: null,
  isSaving: false,

  load: async () => {
    const config = await getConfig();
    set({ config, draft: { ...config }, error: null });
  },

  update: (partial) => {
    const { draft } = get();
    if (!draft) return;
    set({ draft: { ...draft, ...partial }, error: null });
  },

  save: async () => {
    const { draft } = get();
    if (!draft) return;
    set({ isSaving: true, error: null });
    try {
      await setConfig(draft);
      set({ config: { ...draft }, isSaving: false });
    } catch (e) {
      set({ error: String(e), isSaving: false });
    }
  },
}));
