import { create } from "zustand";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  checkForUpdate as checkForUpdateCommand,
  installUpdate as installUpdateCommand,
} from "../lib/ipc";

export interface UpdateInfo {
  version: string;
  notes: string;
  pubDate: string;
  installType: "appimage" | "system_pkg";
}

type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "error";

interface UpdateState {
  status: UpdateStatus;
  update: UpdateInfo | null;
  downloadProgress: number;
  error: string | null;

  setAvailableUpdate: (update: UpdateInfo) => void;
  checkForUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdateStore = create<UpdateState>((set, get) => ({
  status: "idle",
  update: null,
  downloadProgress: 0,
  error: null,

  setAvailableUpdate: (update) => {
    set({ status: "available", update, error: null, downloadProgress: 0 });
  },

  checkForUpdate: async () => {
    set({ status: "checking", error: null });

    try {
      const result = await checkForUpdateCommand();
      if (result) {
        set({ status: "available", update: result, downloadProgress: 0, error: null });
      } else {
        set({ status: "idle", update: null, downloadProgress: 0, error: null });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ status: "error", error: message, downloadProgress: 0 });
    }
  },

  installUpdate: async () => {
    const { update } = get();
    if (!update) return;

    if (update.installType === "appimage") {
      set({ status: "downloading", downloadProgress: 0, error: null });

      try {
        const pendingUpdate = await check();
        if (!pendingUpdate) {
          throw new Error("No update is available to install.");
        }

        let downloaded = 0;
        let total = 0;

        await pendingUpdate.downloadAndInstall((event) => {
          switch (event.event) {
            case "Started":
              total = event.data.contentLength ?? 0;
              downloaded = 0;
              set({ downloadProgress: 0 });
              break;
            case "Progress":
              downloaded += event.data.chunkLength;
              set({
                downloadProgress:
                  total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0,
              });
              break;
            case "Finished":
              set({ downloadProgress: 100 });
              break;
          }
        });

        await relaunch();
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        set({ status: "error", error: message });
      }

      return;
    }

    try {
      await installUpdateCommand();
      set({ status: "idle", update: null, error: null, downloadProgress: 0 });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ status: "error", error: message });
    }
  },

  dismiss: () => set({ status: "idle", update: null, error: null, downloadProgress: 0 }),
}));
