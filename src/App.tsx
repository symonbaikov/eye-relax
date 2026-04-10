import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import OverlayWindow from "./components/OverlayWindow";
import PromptWindow from "./components/PromptWindow";
import SettingsPanel from "./components/SettingsPanel";

const windowLabel = getCurrentWebviewWindow().label;

function App() {
  if (windowLabel === "settings") return <SettingsPanel />;
  if (windowLabel === "prompt") return <PromptWindow />;
  return <OverlayWindow />;
}

export default App;
