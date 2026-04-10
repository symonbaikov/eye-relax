import type { AppConfig, Theme } from "../../../lib/ipc";
import { ToggleRow } from "../controls";

interface AppearancePageProps {
  draft: AppConfig;
  update: (partial: Partial<AppConfig>) => void;
}

export default function AppearancePage({ draft, update }: AppearancePageProps) {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-gray-900">Appearance</h2>
        <p className="text-sm text-gray-500 mt-1">Theme and startup preferences</p>
      </div>

      {/* Theme */}
      <div className="space-y-3">
        <p className="text-sm font-medium text-gray-800">Theme</p>
        <div className="flex gap-2">
          {(["light", "dark", "system"] as Theme[]).map((t) => (
            <button
              key={t}
              onClick={() => update({ theme: t })}
              className={`flex-1 py-2.5 rounded-lg text-sm font-medium capitalize transition-colors ${
                draft.theme === t
                  ? "bg-green-500 text-white"
                  : "bg-gray-100 text-gray-600 hover:bg-gray-200"
              }`}
            >
              {t}
            </button>
          ))}
        </div>
      </div>

      {/* Autostart */}
      <div className="space-y-4">
        <ToggleRow
          label="Autostart"
          description="Launch LookAway automatically at login"
          checked={draft.autostart}
          onChange={(v) => update({ autostart: v })}
        />
      </div>
    </div>
  );
}
