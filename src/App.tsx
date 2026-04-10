import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import OverlayWindow from "./components/OverlayWindow";
import SettingsPanel from "./components/SettingsPanel";

const windowLabel = getCurrentWebviewWindow().label;

function App() {
  if (windowLabel === "settings") return <SettingsPanel />;
  return <OverlayWindow />;
}

export default App;
