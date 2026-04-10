import type { ReactNode } from "react";
import { useAppVersion } from "../../hooks/useAppVersion";

export type Page = "general" | "timers" | "sound" | "appearance" | "statistics" | "about";

interface SidebarProps {
  active: Page;
  onChange: (page: Page) => void;
}

interface NavItem {
  id: Page;
  label: string;
  icon: ReactNode;
}

// Inline SVG icons (16×16)
const icons = {
  general: (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <circle cx="8" cy="8" r="6" />
      <path d="M8 5v3l2 1.5" />
    </svg>
  ),
  timers: (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="2" y="2" width="12" height="12" rx="2" />
      <path d="M5 8h6M8 5v6" />
    </svg>
  ),
  sound: (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M3 6h2l3-3v10L5 10H3a1 1 0 01-1-1V7a1 1 0 011-1z" />
      <path d="M11 5.5a3 3 0 010 5" />
    </svg>
  ),
  appearance: (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <circle cx="8" cy="8" r="3" />
      <path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41" />
    </svg>
  ),
  statistics: (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="2" y="8" width="3" height="6" rx="0.5" />
      <rect x="6.5" y="4" width="3" height="10" rx="0.5" />
      <rect x="11" y="2" width="3" height="12" rx="0.5" />
    </svg>
  ),
  about: (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <circle cx="8" cy="8" r="6" />
      <path d="M8 11V7M8 5h.01" />
    </svg>
  ),
};

const navItems: NavItem[] = [
  { id: "general", label: "General", icon: icons.general },
  { id: "timers", label: "Timers", icon: icons.timers },
  { id: "sound", label: "Sound", icon: icons.sound },
  { id: "appearance", label: "Appearance", icon: icons.appearance },
  { id: "statistics", label: "Statistics", icon: icons.statistics },
  { id: "about", label: "About", icon: icons.about },
];

export default function Sidebar({ active, onChange }: SidebarProps) {
  const version = useAppVersion();

  return (
    <div className="w-[220px] shrink-0 bg-transparent flex flex-col h-full py-4 px-3">
      {/* Brand */}
      <div className="px-4 mb-6 mt-2 flex items-center gap-3">
        <img src="/logo.png" alt="Blinkly Logo" className="w-8 h-8 drop-shadow-sm rounded-lg" />
        <h2 className="text-xl font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-pink-500 to-blue-500">
          Blinkly
        </h2>
      </div>

      {/* Nav items */}
      <nav className="flex-1 space-y-1">
        {navItems.map((item) => (
          <button
            key={item.id}
            onClick={() => onChange(item.id)}
            className={`w-full flex items-center gap-3 px-4 py-2.5 rounded-2xl text-sm font-bold transition-all duration-200 ${
              active === item.id
                ? "bg-white/60 text-blue-600 shadow-sm backdrop-blur-md border border-white/50 translate-x-1"
                : "text-gray-600 hover:bg-white/40 hover:text-gray-900 hover:translate-x-0.5 border border-transparent"
            }`}
          >
            <span
              className={`shrink-0 transition-colors ${active === item.id ? "text-pink-500" : ""}`}
            >
              {item.icon}
            </span>
            {item.label}
          </button>
        ))}
      </nav>

      {/* Footer */}
      <div className="px-4 py-4">
        <p className="text-xs font-bold text-gray-400">v{version}</p>
      </div>
    </div>
  );
}
