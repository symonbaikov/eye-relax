import { create } from "zustand";
import { skipBreak, snoozeBreak } from "../lib/ipc";

interface SchedulerStore {
  remaining: number;
  total: number;
  breakType: "short" | "long" | null;
  isBreakActive: boolean;

  startBreak: (breakType: "short" | "long", totalSecs: number) => void;
  setRemaining: (secs: number) => void;
  endBreak: () => void;

  skip: () => Promise<void>;
  snooze: (durationSecs?: number) => Promise<void>;
}

export const useSchedulerStore = create<SchedulerStore>((set, get) => ({
  remaining: 0,
  total: 0,
  breakType: null,
  isBreakActive: false,

  startBreak: (breakType, totalSecs) =>
    set({ breakType, total: totalSecs, remaining: totalSecs, isBreakActive: true }),

  setRemaining: (secs) => set({ remaining: secs }),

  endBreak: () => set({ isBreakActive: false, breakType: null }),

  skip: async () => {
    // Optimistically end break in UI; backend will confirm via break-completed event
    get().endBreak();
    await skipBreak();
  },

  snooze: async (durationSecs = 300) => {
    get().endBreak();
    await snoozeBreak(durationSecs);
  },
}));
