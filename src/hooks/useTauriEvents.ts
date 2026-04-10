import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSchedulerStore } from "../stores/useSchedulerStore";
import { getConfig, getState, getRemaining } from "../lib/ipc";

export function useTauriEvents() {
  const startBreak = useSchedulerStore((s) => s.startBreak);
  const setRemaining = useSchedulerStore((s) => s.setRemaining);
  const endBreak = useSchedulerStore((s) => s.endBreak);

  useEffect(() => {
    let cancelled = false;
    const unlisten: Array<() => void> = [];

    const setup = async () => {
      // Register listeners first, then check initial state.
      // This order avoids the race where the event arrives while we're still awaiting listen().
      const [unlistenDue, unlistenTick, unlistenDone] = await Promise.all([
        listen<{ breakType: "short" | "long" }>("break-due", async (event) => {
          const config = await getConfig();
          const totalSecs =
            event.payload.breakType === "long"
              ? config.long_break_duration_secs
              : config.break_duration_secs;
          startBreak(event.payload.breakType, totalSecs);
        }),
        listen<{ remainingSecs: number }>("break-tick", (event) => {
          setRemaining(event.payload.remainingSecs);
        }),
        listen("break-completed", () => {
          endBreak();
        }),
      ]);

      if (cancelled) {
        unlistenDue();
        unlistenTick();
        unlistenDone();
        return;
      }

      unlisten.push(unlistenDue, unlistenTick, unlistenDone);

      // After listeners are ready, check if we're already on a break
      // (handles the case where break-due was emitted before React mounted).
      try {
        const state = await getState();
        if (state === "on_break" && !cancelled) {
          const config = await getConfig();
          const remaining = await getRemaining();
          // We don't know the break type here, default to short.
          const totalSecs = config.break_duration_secs;
          startBreak("short", totalSecs);
          setRemaining(remaining);
        }
      } catch {
        // Not critical — if this fails, the next break-tick will sync us.
      }
    };

    setup();

    return () => {
      cancelled = true;
      unlisten.forEach((fn) => fn());
    };
  }, [startBreak, setRemaining, endBreak]);
}
