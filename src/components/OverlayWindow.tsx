import { useCallback, useEffect, useRef, useState } from "react";
import { useTauriEvents } from "../hooks/useTauriEvents";
import { suspendSystem } from "../lib/ipc";
import { useSchedulerStore } from "../stores/useSchedulerStore";
import { getConfig, getRemaining, getState } from "../lib/ipc";

const DOUBLE_ESC_WINDOW_MS = 900;

function CurrentTime() {
  const [time, setTime] = useState(new Date());
  useEffect(() => {
    const id = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(id);
  }, []);
  const h = String(time.getHours()).padStart(2, "0");
  const m = String(time.getMinutes()).padStart(2, "0");
  return (
    <span>
      {h}:{m}
    </span>
  );
}

function TimeDisplay({ remaining }: { remaining: number }) {
  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return (
    <span>
      {String(mins).padStart(2, "0")}:{String(secs).padStart(2, "0")}
    </span>
  );
}

export default function OverlayWindow() {
  useTauriEvents();

  const startBreak = useSchedulerStore((s) => s.startBreak);
  const endBreak = useSchedulerStore((s) => s.endBreak);
  const setRemaining = useSchedulerStore((s) => s.setRemaining);
  const isBreakActive = useSchedulerStore((s) => s.isBreakActive);
  const remaining = useSchedulerStore((s) => s.remaining);
  const skip = useSchedulerStore((s) => s.skip);
  const skipsRemaining = useSchedulerStore((s) => s.skipsRemaining);
  const skipLimit = useSchedulerStore((s) => s.skipLimit);
  const refreshSkipAllowance = useSchedulerStore((s) => s.refreshSkipAllowance);

  const [visible, setVisible] = useState(false);
  const [isSuspending, setIsSuspending] = useState(false);
  const [suspendError, setSuspendError] = useState<string | null>(null);
  const [suspendStatus, setSuspendStatus] = useState<string | null>(null);
  const [skipStatus, setSkipStatus] = useState<string | null>(null);
  const [skipError, setSkipError] = useState<string | null>(null);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastEscAtRef = useRef<number | null>(null);

  useEffect(() => {
    if (isBreakActive) {
      if (hideTimer.current) clearTimeout(hideTimer.current);
      const t = setTimeout(() => setVisible(true), 150);
      return () => clearTimeout(t);
    } else {
      setVisible(false);
    }
  }, [isBreakActive]);

  useEffect(() => {
    const poll = async () => {
      try {
        const state = await getState();
        const isActive = useSchedulerStore.getState().isBreakActive;
        if (state === "on_break") {
          const rem = await getRemaining();
          if (!isActive) {
            const config = await getConfig();
            startBreak("short", config.break_duration_secs);
          }
          setRemaining(rem);
        } else if (isActive) {
          endBreak();
        }
      } catch (e) {
        console.error("Failed to poll state:", e);
      }
    };

    const id = setInterval(() => {
      void poll();
    }, 500);
    void poll();
    return () => clearInterval(id);
  }, [startBreak, endBreak, setRemaining]);

  useEffect(() => {
    void refreshSkipAllowance();
  }, [refreshSkipAllowance]);

  const prefersReducedMotion =
    typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const duration = prefersReducedMotion ? 0 : 3;

  const handleSkipBreak = useCallback(async () => {
    if (skipsRemaining <= 0) {
      setSkipError(`Daily skip limit reached (${skipLimit} per day).`);
      setSkipStatus(null);
      return;
    }

    try {
      setSkipError(null);
      setSkipStatus("Skipping break...");
      await skip();
      setSkipStatus("Break skipped");
    } catch (error) {
      console.error("Failed to skip break:", error);
      const message = error instanceof Error ? error.message : String(error);
      setSkipError(message);
      setSkipStatus(null);
    }
  }, [skip, skipLimit, skipsRemaining]);

  const handleSuspendSystem = async () => {
    if (isSuspending) return;

    try {
      setIsSuspending(true);
      setSuspendError(null);
      setSuspendStatus("Sending suspend request...");
      await suspendSystem();
      setSuspendStatus("Suspend request sent");
    } catch (error) {
      console.error("Failed to suspend system:", error);
      const message = error instanceof Error ? error.message : String(error);
      setSuspendError(message);
      setSuspendStatus(null);
    } finally {
      setIsSuspending(false);
    }
  };

  useEffect(() => {
    if (!suspendStatus) return;
    const timeout = window.setTimeout(() => setSuspendStatus(null), 2500);
    return () => window.clearTimeout(timeout);
  }, [suspendStatus]);

  useEffect(() => {
    if (!skipStatus) return;
    const timeout = window.setTimeout(() => setSkipStatus(null), 1800);
    return () => window.clearTimeout(timeout);
  }, [skipStatus]);

  useEffect(() => {
    if (!isBreakActive) {
      lastEscAtRef.current = null;
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;

      const now = Date.now();
      const previous = lastEscAtRef.current;

      if (previous && now - previous <= DOUBLE_ESC_WINDOW_MS) {
        lastEscAtRef.current = null;
        void handleSkipBreak();
        return;
      }

      lastEscAtRef.current = now;
      setSkipError(null);
      setSkipStatus("Press Esc again to skip");
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [handleSkipBreak, isBreakActive]);

  return (
    <div
      className="overlay-gradient-bg h-screen w-screen overflow-hidden select-none relative flex flex-col items-center"
      style={{
        fontFamily: "'Nunito', sans-serif",
        opacity: visible ? 1 : 0,
        transition: `opacity ${duration}s ease-in-out`,
        pointerEvents: visible ? "auto" : "none",
        backdropFilter: "blur(20px)",
      }}
    >
      <div className="absolute inset-0 overflow-hidden pointer-events-none z-0 mix-blend-screen opacity-50">
        <div className="overlay-blob-1 absolute top-1/4 left-1/4 w-[30rem] h-[30rem] bg-fuchsia-400 rounded-full blur-[100px]" />
        <div className="overlay-blob-2 absolute top-1/3 right-1/4 w-[35rem] h-[35rem] bg-cyan-400 rounded-full blur-[120px]" />
        <div className="overlay-blob-3 absolute bottom-1/4 left-1/3 w-[25rem] h-[25rem] bg-blue-500 rounded-full blur-[90px]" />
      </div>

      <div className="absolute top-12 flex items-center gap-1.5 text-white/90 font-medium text-sm tracking-wide z-10">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <circle cx="12" cy="12" r="10"></circle>
          <polyline points="12 6 12 12 16 14"></polyline>
        </svg>
        <CurrentTime />
      </div>

      <div className="flex-1 flex flex-col items-center justify-center -mt-16 w-full z-10">
        <h1 className="text-white font-extrabold tracking-tight mb-4" style={{ fontSize: "56px" }}>
          Eyes to the horizon
        </h1>

        <p className="text-white/90 text-xl font-bold mb-10 text-center tracking-wide">
          Set your eyes on something distant until the countdown is over
        </p>

        <div className="w-24 h-[3px] bg-white/40 mb-8 rounded-full"></div>

        <div
          className="font-black tracking-wider tabular-nums"
          style={{
            fontSize: "72px",
            color: "white",
            textShadow: "0 0 30px rgba(255, 255, 255, 0.5)",
          }}
        >
          <TimeDisplay remaining={remaining} />
        </div>
      </div>

      <div className="absolute bottom-16 flex flex-col items-center gap-4 z-10">
        <div className="flex gap-4">
          <button
            onClick={() => {
              void handleSkipBreak();
            }}
            className="flex items-center gap-2 px-6 py-3 rounded-full font-semibold text-sm cursor-pointer border border-white/20 transition-all hover:bg-white/20 active:bg-white/30"
            style={{
              background: "rgba(30, 60, 100, 0.5)",
              color: "white",
              backdropFilter: "blur(10px)",
              boxShadow: "0 4px 12px rgba(0,0,0,0.2)",
              opacity: skipsRemaining <= 0 ? 0.55 : 1,
            }}
            disabled={skipsRemaining <= 0}
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <polyline points="13 17 18 12 13 7"></polyline>
              <polyline points="6 17 11 12 6 7"></polyline>
            </svg>
            Skip Break
          </button>

          <button
            onClick={() => {
              void handleSuspendSystem();
            }}
            disabled={isSuspending}
            className="flex items-center gap-2 px-6 py-3 rounded-full font-semibold text-sm cursor-pointer border border-white/20 transition-all hover:bg-white/20 active:bg-white/30"
            style={{
              background: "rgba(30, 50, 90, 0.5)",
              color: "white",
              backdropFilter: "blur(10px)",
              boxShadow: "0 4px 12px rgba(0,0,0,0.2)",
              opacity: isSuspending ? 0.7 : 1,
            }}
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
              <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
            </svg>
            {isSuspending ? "Suspending..." : "Suspend"}
          </button>
        </div>

        <div className="text-center flex flex-col gap-1.5 mt-2">
          {skipStatus ? (
            <p className="text-[11px] font-medium text-sky-200/90">{skipStatus}</p>
          ) : null}
          {skipError ? (
            <p className="max-w-md text-[11px] leading-5 font-medium text-amber-200/90">
              {skipError}
            </p>
          ) : null}
          {suspendStatus ? (
            <p className="text-[11px] font-medium text-sky-200/90">{suspendStatus}</p>
          ) : null}
          {suspendError ? (
            <p className="max-w-md text-[11px] leading-5 font-medium text-amber-200/90">
              {suspendError}
            </p>
          ) : null}
          <p className="text-white/50 text-xs font-medium">
            {skipsRemaining} of {skipLimit} skips left today
          </p>
          <p className="text-white/50 text-xs font-medium flex items-center gap-1.5 justify-center">
            Press{" "}
            <kbd className="px-1.5 py-0.5 rounded bg-white/10 border border-white/20 font-sans text-[10px] text-white/80">
              Esc
            </kbd>{" "}
            twice to skip the break
          </p>
        </div>
      </div>
    </div>
  );
}
