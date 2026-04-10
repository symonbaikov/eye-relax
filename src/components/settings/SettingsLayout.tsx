import { useEffect, useState } from "react";
import { useConfigStore } from "../../stores/useConfigStore";
import Sidebar, { type Page } from "./Sidebar";
import GeneralPage from "./pages/GeneralPage";
import TimersPage from "./pages/TimersPage";
import SoundPage from "./pages/SoundPage";
import AppearancePage from "./pages/AppearancePage";
import StatsPage from "./pages/StatsPage";
import AboutPage from "./pages/AboutPage";
import UpdateBanner from "./UpdateBanner";

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
    <div className="h-screen flex bg-gradient-to-br from-pink-100/50 via-purple-50/50 to-blue-100/50 text-gray-800 font-sans">
      <Sidebar active={page} onChange={setPage} />

      <div className="flex-1 flex flex-col min-w-0 bg-white/60 backdrop-blur-3xl shadow-[0_0_40px_-10px_rgba(0,0,0,0.1)] rounded-l-[2.5rem] border-l border-white/60 my-2 mr-2 overflow-hidden">
        {/* Content */}
        <div className="flex-1 overflow-y-auto px-8 py-8">
          <UpdateBanner />
          {renderPage()}

          {error && (
            <p className="text-sm text-red-500 bg-red-50/80 backdrop-blur rounded-lg px-3 py-2 mt-4 border border-red-100">
              {error}
            </p>
          )}
        </div>

        {/* Save footer */}
        {showSave && (
          <div className="px-8 py-5 bg-white/40 backdrop-blur-md border-t border-white/50">
            <button
              onClick={() => void save()}
              disabled={isSaving}
              className="w-full py-3 rounded-2xl text-sm font-bold text-white bg-gradient-to-r from-pink-400 to-blue-400 hover:from-pink-500 hover:to-blue-500 shadow-lg shadow-pink-500/25 disabled:opacity-50 transition-all active:scale-[0.98]"
            >
              {isSaving ? "Saving..." : "Save Changes"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
