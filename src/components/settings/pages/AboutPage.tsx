export default function AboutPage() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-gray-900">About</h2>
        <p className="text-sm text-gray-500 mt-1">LookAway for Linux</p>
      </div>

      <div className="bg-gray-50 rounded-xl p-5 space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-sm text-gray-600">Version</span>
          <span className="text-sm font-medium text-gray-900">0.1.0</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm text-gray-600">Framework</span>
          <span className="text-sm font-medium text-gray-900">Tauri 2 + React</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm text-gray-600">License</span>
          <span className="text-sm font-medium text-gray-900">MIT</span>
        </div>
      </div>

      <div className="space-y-2">
        <p className="text-sm text-gray-700">
          LookAway is a gentle eye break reminder that follows the 20-20-20 rule.
          It runs quietly in your system tray and reminds you to take regular breaks
          from your screen.
        </p>
        <p className="text-sm text-gray-500">
          Built with Tauri, Rust, React, and TypeScript.
        </p>
      </div>
    </div>
  );
}
