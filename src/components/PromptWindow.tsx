import { useMemo, useState } from "react";
import { useTauriEvents } from "../hooks/useTauriEvents";
import { useSchedulerStore } from "../stores/useSchedulerStore";

function formatCountdown(totalSecs: number) {
  const mins = Math.floor(totalSecs / 60);
  const secs = totalSecs % 60;
  return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
}

export default function PromptWindow() {
  useTauriEvents();

  const isPromptVisible = useSchedulerStore((s) => s.isPromptVisible);
  const promptRemaining = useSchedulerStore((s) => s.promptRemaining);
  const promptBreakType = useSchedulerStore((s) => s.promptBreakType);
  const deferPrompt = useSchedulerStore((s) => s.deferPrompt);

  const [busyAction, setBusyAction] = useState<number | null>(null);
  const options = useMemo(() => [0, 60, 300, 900], []);

  const handleAction = async (durationSecs: number) => {
    try {
      setBusyAction(durationSecs);
      await deferPrompt(durationSecs);
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <div
      className="h-screen w-screen overflow-hidden"
      style={{
        fontFamily: "'Nunito', sans-serif",
        background: "transparent",
        opacity: isPromptVisible ? 1 : 0,
        pointerEvents: isPromptVisible ? "auto" : "none",
        transition: "opacity 220ms ease",
      }}
    >
      <div className="absolute inset-0 flex items-start justify-center pt-4 px-4">
        <div className="w-full max-w-[460px] rounded-[28px] border border-white/15 bg-[#201a57]/78 shadow-[0_25px_90px_rgba(13,10,45,0.55)] backdrop-blur-2xl px-4 py-3 text-white">
          <div className="absolute inset-0 rounded-[28px] bg-[radial-gradient(circle_at_top_left,rgba(255,116,188,0.22),transparent_36%),radial-gradient(circle_at_top_right,rgba(100,170,255,0.28),transparent_42%)] pointer-events-none" />

          <div className="relative flex items-center gap-3">
            <div className="flex h-11 w-11 items-center justify-center rounded-full bg-gradient-to-br from-pink-400 via-pink-500 to-orange-300 shadow-[0_10px_28px_rgba(255,95,175,0.35)]">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" className="text-white">
                <circle
                  cx="12"
                  cy="12"
                  r="8.5"
                  stroke="currentColor"
                  strokeWidth="1.8"
                  opacity="0.95"
                />
                <path
                  d="M12 7v5l3.5 2"
                  stroke="currentColor"
                  strokeWidth="1.8"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </div>

            <div className="min-w-0 flex-1">
              <div className="text-[18px] leading-none font-extrabold tracking-wide">
                {formatCountdown(promptRemaining)}
              </div>
              <p className="mt-1 text-[14px] leading-5 text-white/72 font-semibold">
                {promptBreakType === "long"
                  ? "A longer reset is coming up. Your focus will thank you."
                  : "Almost time. Your eyes will appreciate this."}
              </p>
            </div>
          </div>

          <div className="relative mt-4 flex items-center gap-2">
            {options.map((durationSecs) => {
              const isPrimary = durationSecs === 0;
              const label =
                durationSecs === 0 ? "Start this break now" : `+${Math.round(durationSecs / 60)}m`;

              return (
                <button
                  key={durationSecs}
                  onClick={() => {
                    void handleAction(durationSecs);
                  }}
                  disabled={busyAction !== null}
                  className={`rounded-full border text-sm font-bold transition-all duration-200 ${
                    isPrimary
                      ? "px-4 h-9 bg-white/10 border-white/10 text-white/90 hover:bg-white/16"
                      : "px-5 h-9 bg-transparent border-white/16 text-white/82 hover:bg-white/8"
                  } ${busyAction === durationSecs ? "opacity-70" : ""}`}
                >
                  {busyAction === durationSecs && durationSecs === 0 ? "Starting..." : label}
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
