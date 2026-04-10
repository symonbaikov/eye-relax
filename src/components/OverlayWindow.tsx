import { useEffect, useRef, useState } from "react";
import { useTauriEvents } from "../hooks/useTauriEvents";
import { useSchedulerStore } from "../stores/useSchedulerStore";
import { getConfig, getRemaining, getState } from "../lib/ipc";

// ---------------------------------------------------------------------------
// Motivational messages
// ---------------------------------------------------------------------------

const MESSAGES = [
  "Look 20 feet away for 20 seconds",
  "Rest your eyes — you've earned it",
  "Focus on something in the distance",
  "Give your eyes a moment to relax",
  "Your future self thanks you",
  "Let your eyes rest on the horizon",
  "A short break keeps eye strain away",
  "Blink a few times and breathe",
  "The 20-20-20 rule keeps eyes healthy",
  "Your eyes deserve a rest too",
];

function TimeDisplay({ remaining }: { remaining: number }) {
  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return (
    <span>
      {String(mins).padStart(2, "0")}:{String(secs).padStart(2, "0")}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export default function OverlayWindow() {
  useTauriEvents();

  const startBreak = useSchedulerStore((s) => s.startBreak);
  const endBreak = useSchedulerStore((s) => s.endBreak);
  const setRemaining = useSchedulerStore((s) => s.setRemaining);
  const isBreakActive = useSchedulerStore((s) => s.isBreakActive);
  const remaining = useSchedulerStore((s) => s.remaining);
  const total = useSchedulerStore((s) => s.total);
  const skip = useSchedulerStore((s) => s.skip);
  const snooze = useSchedulerStore((s) => s.snooze);

  // CSS opacity state — drives the 3-second fade in/out
  const [visible, setVisible] = useState(false);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (isBreakActive) {
      if (hideTimer.current) clearTimeout(hideTimer.current);
      // Small delay ensures:
      // 1. Window is fully shown by GTK (transparent, so invisible)
      // 2. React has painted the first frame at opacity:0
      // 3. CSS transition then fades in smoothly
      const t = setTimeout(() => setVisible(true), 150);
      return () => clearTimeout(t);
    } else {
      setVisible(false);
    }
  }, [isBreakActive]);

  // Polling: sync remaining & detect on_break state every 500ms
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
      } catch {
        /* transient error — next tick will retry */
      }
    };

    const id = setInterval(() => { void poll(); }, 500);
    void poll();
    return () => clearInterval(id);
  }, [startBreak, endBreak, setRemaining]);

  const elapsed = total - remaining;
  const msgIndex =
    total > 0
      ? Math.floor((elapsed / total) * MESSAGES.length) % MESSAGES.length
      : 0;
  const message = MESSAGES[Math.max(0, Math.min(msgIndex, MESSAGES.length - 1))];

  const prefersReducedMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  const duration = prefersReducedMotion ? 0 : 3;

  return (
    <div
      className="overlay-gradient-bg h-screen w-screen overflow-hidden select-none flex items-center justify-center relative"
      style={{
        opacity: visible ? 1 : 0,
        transition: `opacity ${duration}s ease-in-out`,
        pointerEvents: visible ? "auto" : "none",
      }}
    >

      {/* Ambient blobs */}
      <div
        className="overlay-blob-1 absolute rounded-full pointer-events-none"
        style={{
          width: 900, height: 900, top: -350, left: -300,
          background: "radial-gradient(circle, rgba(190,100,180,0.28) 0%, transparent 70%)",
          filter: "blur(100px)",
        }}
      />
      <div
        className="overlay-blob-2 absolute rounded-full pointer-events-none"
        style={{
          width: 700, height: 700, bottom: -250, right: -250,
          background: "radial-gradient(circle, rgba(140,80,200,0.22) 0%, transparent 70%)",
          filter: "blur(90px)",
        }}
      />
      <div
        className="overlay-blob-3 absolute rounded-full pointer-events-none"
        style={{
          width: 600, height: 600, top: "25%", left: "50%",
          background: "radial-gradient(circle, rgba(220,120,160,0.18) 0%, transparent 70%)",
          filter: "blur(110px)",
        }}
      />

      {/* Content */}
      <div className="relative flex flex-col items-center gap-6 px-8 text-center z-10">
        <h1
          className="font-semibold tracking-tight"
          style={{ color: "rgba(255,255,255,0.95)", fontSize: 38 }}
        >
          Eyes to the horizon
        </h1>

        <p
          className="leading-relaxed max-w-sm"
          style={{ color: "rgba(255,255,255,0.52)", fontSize: 16 }}
        >
          {message}
        </p>

        <div
          className="font-mono font-light tabular-nums"
          style={{
            fontSize: 80,
            color: "rgba(255,255,255,0.92)",
            letterSpacing: "0.04em",
            lineHeight: 1,
          }}
        >
          <TimeDisplay remaining={remaining} />
        </div>

        <div className="flex gap-3 mt-3">
          <button
            onClick={() => void snooze()}
            className="px-6 py-2.5 rounded-full text-sm font-medium cursor-pointer"
            style={{
              background: "rgba(255,255,255,0.10)",
              color: "rgba(255,255,255,0.65)",
              border: "1px solid rgba(255,255,255,0.18)",
              transition: "background 0.2s",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.18)"; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.10)"; }}
          >
            Snooze 5m
          </button>
          <button
            onClick={() => void skip()}
            className="px-6 py-2.5 rounded-full text-sm font-medium cursor-pointer"
            style={{
              background: "rgba(255,255,255,0.20)",
              color: "rgba(255,255,255,0.92)",
              border: "1px solid rgba(255,255,255,0.28)",
              transition: "background 0.2s",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.30)"; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = "rgba(255,255,255,0.20)"; }}
          >
            Skip Break
          </button>
        </div>
      </div>
    </div>
  );
}
