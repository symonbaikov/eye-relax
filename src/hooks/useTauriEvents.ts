import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSchedulerStore } from "../stores/useSchedulerStore";
import { getConfig, getState, getRemaining } from "../lib/ipc";

export function useTauriEvents() {
  const startBreak = useSchedulerStore((s) => s.startBreak);
  const setRemaining = useSchedulerStore((s) => s.setRemaining);
  const endBreak = useSchedulerStore((s) => s.endBreak);
  const showPrompt = useSchedulerStore((s) => s.showPrompt);
  const hidePrompt = useSchedulerStore((s) => s.hidePrompt);

  useEffect(() => {
    let cancelled = false;
    const unlisten: Array<() => void> = [];

    const setup = async () => {
      // Register listeners first, then check initial state.
      // This order avoids the race where the event arrives while we're still awaiting listen().
      const [unlistenPromptTick, unlistenPromptHide, unlistenDue, unlistenTick, unlistenDone] =
        await Promise.all([
          listen<{ breakType: "short" | "long"; remainingSecs: number }>(
            "pre-break-prompt-tick",
            (event) => {
              showPrompt(event.payload.breakType, event.payload.remainingSecs);
            }
          ),
          listen("pre-break-prompt-hide", () => {
            hidePrompt();
          }),
          listen<{ breakType: "short" | "long" }>("break-due", async (event) => {
            hidePrompt();
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
            hidePrompt();
            endBreak();
          }),
        ]);

      if (cancelled) {
        unlistenPromptTick();
        unlistenPromptHide();
        unlistenDue();
        unlistenTick();
        unlistenDone();
        return;
      }

      unlisten.push(
        unlistenPromptTick,
        unlistenPromptHide,
        unlistenDue,
        unlistenTick,
        unlistenDone
      );

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
  }, [startBreak, setRemaining, endBreak, showPrompt, hidePrompt]);
}
