import { useEffect } from "react";
import { useUpdateStore } from "../../stores/useUpdateStore";

export default function UpdateBanner() {
  const { status, update, downloadProgress, error, checkForUpdate, installUpdate, dismiss } =
    useUpdateStore();

  useEffect(() => {
    void checkForUpdate();
  }, [checkForUpdate]);

  if (status === "idle" || status === "checking") {
    return null;
  }

  if (status === "error") {
    return (
      <div className="mb-4 rounded-3xl border border-red-200/80 bg-red-50/90 px-4 py-3 text-sm text-red-600 shadow-sm backdrop-blur-md">
        Failed to check for updates: {error}
        <button onClick={dismiss} className="ml-2 font-semibold underline underline-offset-2">
          Dismiss
        </button>
      </div>
    );
  }

  if (status === "available" && update) {
    return (
      <div className="mb-5 rounded-[1.75rem] border border-blue-200/70 bg-gradient-to-r from-blue-50/95 via-white/90 to-pink-50/95 px-5 py-4 shadow-[0_18px_50px_-28px_rgba(47,104,206,0.45)] backdrop-blur-xl">
        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            <p className="text-sm font-semibold text-gray-800">
              Blinkly {update.version} is available
            </p>
            <p className="mt-1 text-xs leading-5 text-gray-500">
              {update.notes ||
                (update.installType === "appimage"
                  ? "A new AppImage is ready to download and install automatically."
                  : "A newer Linux package is available on the releases page.")}
            </p>
          </div>

          <div className="flex items-center gap-2 self-start md:self-auto">
            <button
              onClick={dismiss}
              className="rounded-full px-3 py-1.5 text-xs font-semibold text-gray-500 transition-colors hover:text-gray-700"
            >
              Later
            </button>
            <button
              onClick={() => {
                void installUpdate();
              }}
              className="rounded-full bg-gradient-to-r from-pink-400 to-blue-400 px-4 py-2 text-xs font-bold text-white shadow-lg shadow-pink-500/20 transition-all hover:from-pink-500 hover:to-blue-500 active:scale-[0.98]"
            >
              {update.installType === "appimage" ? "Update" : "Download"}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (status === "downloading") {
    return (
      <div className="mb-5 rounded-[1.75rem] border border-blue-200/70 bg-blue-50/90 px-5 py-4 shadow-[0_18px_50px_-28px_rgba(47,104,206,0.45)] backdrop-blur-xl">
        <div className="flex items-center gap-3">
          <div className="flex-1">
            <p className="text-sm font-semibold text-gray-800">Downloading update...</p>
            <div className="mt-2 h-2 overflow-hidden rounded-full bg-blue-100">
              <div
                className="h-full rounded-full bg-gradient-to-r from-pink-400 to-blue-400 transition-all duration-300"
                style={{ width: `${downloadProgress}%` }}
              />
            </div>
          </div>
          <span className="text-xs font-semibold tabular-nums text-gray-500">
            {downloadProgress}%
          </span>
        </div>
      </div>
    );
  }

  return null;
}
