import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import OverlayWindow from "./components/OverlayWindow";
import PromptWindow from "./components/PromptWindow";
import SettingsPanel from "./components/SettingsPanel";
import type { UpdateInfo } from "./stores/useUpdateStore";
import { useUpdateStore } from "./stores/useUpdateStore";

const windowLabel = getCurrentWebviewWindow().label;

function App() {
  useEffect(() => {
    if (windowLabel !== "settings") {
      return;
    }

    let disposed = false;
    let cleanup: (() => void) | null = null;

    void listen<UpdateInfo>("update-available", (event) => {
      useUpdateStore.getState().setAvailableUpdate(event.payload);
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
        return;
      }

      cleanup = unlisten;
    });

    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);

  if (windowLabel === "settings") return <SettingsPanel />;
  if (windowLabel === "prompt") return <PromptWindow />;
  return <OverlayWindow />;
}

export default App;
