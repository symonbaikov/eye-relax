import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

export function useAppVersion(fallback = "0.1.0") {
  const [version, setVersion] = useState(fallback);

  useEffect(() => {
    let cancelled = false;

    const loadVersion = async () => {
      try {
        const appVersion = await getVersion();
        if (!cancelled) {
          setVersion(appVersion);
        }
      } catch {
        // Keep the fallback version when the runtime API is unavailable.
      }
    };

    void loadVersion();

    return () => {
      cancelled = true;
    };
  }, []);

  return version;
}
