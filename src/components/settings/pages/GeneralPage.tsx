import { useEffect, useState } from "react";
import { getState, getRemaining, pauseTimer, resumeTimer, type SchedulerState } from "../../../lib/ipc";

const stateLabels: Record<SchedulerState, string> = {
  idle: "Idle",
  working: "Working",
  on_break: "On Break",
  paused: "Paused",
};

const stateColors: Record<SchedulerState, string> = {
  idle: "bg-gray-100 text-gray-600",
  working: "bg-green-100 text-green-700",
  on_break: "bg-yellow-100 text-yellow-700",
  paused: "bg-gray-100 text-gray-500",
};

function fmtTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

export default function GeneralPage() {
  const [state, setState] = useState<SchedulerState>("idle");
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    const poll = async () => {
      const [s, r] = await Promise.all([getState(), getRemaining()]);
      setState(s);
      setRemaining(r);
    };
    void poll();
    const id = setInterval(() => void poll(), 1000);
    return () => clearInterval(id);
  }, []);

  const handlePauseResume = async () => {
    if (state === "paused") {
      await resumeTimer();
    } else {
      await pauseTimer();
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-gray-900">General</h2>
        <p className="text-sm text-gray-500 mt-1">Current timer status and controls</p>
      </div>

      {/* Status card */}
      <div className="bg-gray-50 rounded-xl p-5 space-y-4">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-gray-600">Status</span>
          <span className={`px-3 py-1 rounded-full text-xs font-semibold ${stateColors[state]}`}>
            {stateLabels[state]}
          </span>
        </div>

        {(state === "working" || state === "on_break") && (
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-gray-600">Time remaining</span>
            <span className="text-2xl font-semibold text-gray-900 tabular-nums">
              {fmtTime(remaining)}
            </span>
          </div>
        )}

        {state === "paused" && (
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-gray-600">Paused with</span>
            <span className="text-2xl font-semibold text-gray-900 tabular-nums">
              {fmtTime(remaining)}
            </span>
          </div>
        )}
      </div>

      {/* Controls */}
      <div className="flex gap-3">
        {(state === "working" || state === "paused") && (
          <button
            onClick={() => void handlePauseResume()}
            className="flex-1 py-2.5 rounded-xl text-sm font-semibold text-white bg-green-500 hover:bg-green-600 transition-colors"
          >
            {state === "paused" ? "Resume" : "Pause"}
          </button>
        )}
      </div>

      {/* Info */}
      <div className="bg-blue-50 rounded-xl p-4">
        <p className="text-sm text-blue-700">
          LookAway follows the 20-20-20 rule: every 20 minutes, look at something
          20 feet away for 20 seconds. This helps reduce eye strain during long
          work sessions.
        </p>
      </div>
    </div>
  );
}
