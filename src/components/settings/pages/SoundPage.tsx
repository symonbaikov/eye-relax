import type { AppConfig } from "../../../lib/ipc";
import { ToggleRow } from "../controls";

interface SoundPageProps {
  draft: AppConfig;
  update: (partial: Partial<AppConfig>) => void;
}

export default function SoundPage({ draft, update }: SoundPageProps) {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-gray-900">Sound</h2>
        <p className="text-sm text-gray-500 mt-1">Audio notifications for breaks</p>
      </div>

      <div className="space-y-4">
        <ToggleRow
          label="Break sound"
          description="Play a soft chime when a break begins"
          checked={draft.sound_enabled}
          onChange={(v) => update({ sound_enabled: v })}
        />
      </div>
    </div>
  );
}
