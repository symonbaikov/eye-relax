import { create } from "zustand";
import { deferBreak, getSkipAllowance, skipBreak, snoozeBreak } from "../lib/ipc";

interface SchedulerStore {
  remaining: number;
  total: number;
  breakType: "short" | "long" | null;
  isBreakActive: boolean;
  isPromptVisible: boolean;
  promptRemaining: number;
  promptBreakType: "short" | "long" | null;
  skipLimit: number;
  skipsRemaining: number;

  startBreak: (breakType: "short" | "long", totalSecs: number) => void;
  showPrompt: (breakType: "short" | "long", remainingSecs: number) => void;
  hidePrompt: () => void;
  setRemaining: (secs: number) => void;
  endBreak: () => void;
  refreshSkipAllowance: () => Promise<void>;

  skip: () => Promise<void>;
  snooze: (durationSecs?: number) => Promise<void>;
  deferPrompt: (durationSecs: number) => Promise<void>;
}

export const useSchedulerStore = create<SchedulerStore>((set, get) => ({
  remaining: 0,
  total: 0,
  breakType: null,
  isBreakActive: false,
  isPromptVisible: false,
  promptRemaining: 0,
  promptBreakType: null,
  skipLimit: 4,
  skipsRemaining: 4,

  startBreak: (breakType, totalSecs) =>
    set({
      breakType,
      total: totalSecs,
      remaining: totalSecs,
      isBreakActive: true,
      isPromptVisible: false,
      promptRemaining: 0,
      promptBreakType: null,
    }),

  showPrompt: (breakType, remainingSecs) =>
    set({ isPromptVisible: true, promptBreakType: breakType, promptRemaining: remainingSecs }),

  hidePrompt: () => set({ isPromptVisible: false, promptBreakType: null, promptRemaining: 0 }),

  setRemaining: (secs) => set({ remaining: secs }),

  endBreak: () => set({ isBreakActive: false, breakType: null }),

  refreshSkipAllowance: async () => {
    const allowance = await getSkipAllowance();
    set({ skipsRemaining: allowance.remaining, skipLimit: allowance.limit });
  },

  skip: async () => {
    await skipBreak();
    await get().refreshSkipAllowance();
    get().endBreak();
  },

  snooze: async (durationSecs = 300) => {
    get().endBreak();
    await snoozeBreak(durationSecs);
    await get().refreshSkipAllowance();
  },

  deferPrompt: async (durationSecs) => {
    get().hidePrompt();
    await deferBreak(durationSecs);
  },
}));
