import type { AppConfig } from "../../../lib/ipc";
import { SliderRow } from "../controls";

const fmtMins = (secs: number) => {
  const m = Math.round(secs / 60);
  return m === 1 ? "1 min" : `${m} min`;
};

const fmtSecs = (secs: number) => `${secs}s`;

interface TimersPageProps {
  draft: AppConfig;
  update: (partial: Partial<AppConfig>) => void;
}

export default function TimersPage({ draft, update }: TimersPageProps) {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-gray-900">Timers</h2>
        <p className="text-sm text-gray-500 mt-1">Configure work and break intervals</p>
      </div>

      <div className="space-y-5">
        <SliderRow
          label="Work interval"
          value={draft.work_interval_secs}
          min={300}
          max={3600}
          step={60}
          format={fmtMins}
          onChange={(v) => update({ work_interval_secs: v })}
        />
        <SliderRow
          label="Short break duration"
          value={draft.break_duration_secs}
          min={10}
          max={60}
          step={5}
          format={fmtSecs}
          onChange={(v) => update({ break_duration_secs: v })}
        />
        <SliderRow
          label="Long break interval"
          value={draft.long_break_interval_secs}
          min={1800}
          max={7200}
          step={300}
          format={fmtMins}
          onChange={(v) => update({ long_break_interval_secs: v })}
        />
        <SliderRow
          label="Long break duration"
          value={draft.long_break_duration_secs}
          min={120}
          max={900}
          step={60}
          format={fmtMins}
          onChange={(v) => update({ long_break_duration_secs: v })}
        />
        <SliderRow
          label="Snooze duration"
          value={draft.snooze_duration_secs}
          min={60}
          max={600}
          step={60}
          format={fmtMins}
          onChange={(v) => update({ snooze_duration_secs: v })}
        />
        <SliderRow
          label="Idle threshold"
          value={draft.idle_threshold_secs}
          min={120}
          max={900}
          step={60}
          format={fmtMins}
          onChange={(v) => update({ idle_threshold_secs: v })}
        />
      </div>
    </div>
  );
}
