import { useEffect, useState } from "react";
import { useConfigStore } from "../../stores/useConfigStore";
import Sidebar, { type Page } from "./Sidebar";
import GeneralPage from "./pages/GeneralPage";
import TimersPage from "./pages/TimersPage";
import SoundPage from "./pages/SoundPage";
import AppearancePage from "./pages/AppearancePage";
import StatsPage from "./pages/StatsPage";
import AboutPage from "./pages/AboutPage";

const pagesWithSave = new Set<Page>(["timers", "sound", "appearance"]);

export default function SettingsLayout() {
  const [page, setPage] = useState<Page>("general");
  const { draft, error, isSaving, load, update, save } = useConfigStore();

  useEffect(() => {
    void load();
  }, [load]);

  if (!draft) {
    return (
      <div className="h-screen flex items-center justify-center bg-white">
        <p className="text-sm text-gray-400">Loading...</p>
      </div>
    );
  }

  const renderPage = () => {
    switch (page) {
      case "general":
        return <GeneralPage />;
      case "timers":
        return <TimersPage draft={draft} update={update} />;
      case "sound":
        return <SoundPage draft={draft} update={update} />;
      case "appearance":
        return <AppearancePage draft={draft} update={update} />;
      case "statistics":
        return <StatsPage />;
      case "about":
        return <AboutPage />;
    }
  };

  const showSave = pagesWithSave.has(page);

  return (
    <div className="h-screen flex bg-white">
      <Sidebar active={page} onChange={setPage} />

      <div className="flex-1 flex flex-col min-w-0">
        {/* Content */}
        <div className="flex-1 overflow-y-auto px-6 py-5">
          {renderPage()}

          {error && (
            <p className="text-sm text-red-500 bg-red-50 rounded-lg px-3 py-2 mt-4">{error}</p>
          )}
        </div>

        {/* Save footer */}
        {showSave && (
          <div className="px-6 py-4 border-t border-gray-100">
            <button
              onClick={() => void save()}
              disabled={isSaving}
              className="w-full py-2.5 rounded-xl text-sm font-semibold text-white bg-green-500 hover:bg-green-600 disabled:opacity-50 transition-colors"
            >
              {isSaving ? "Saving..." : "Save"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
