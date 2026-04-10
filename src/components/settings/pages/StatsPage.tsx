import { useEffect, useState } from "react";
import { type DateRange, type DayStat, getStats } from "../../../lib/ipc";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function fmtTime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function getLast7Days(): DateRange {
  const end = new Date();
  const start = new Date();
  start.setDate(end.getDate() - 6);
  const fmt = (d: Date) => d.toISOString().slice(0, 10);
  return { start: fmt(start), end: fmt(end) };
}

function getLabel(date: string): string {
  const d = new Date(date + "T00:00:00");
  return d.toLocaleDateString(undefined, { weekday: "short" });
}

// ---------------------------------------------------------------------------
// Bar chart
// ---------------------------------------------------------------------------

function BarChart({ data }: { data: DayStat[] }) {
  const maxWork = Math.max(...data.map((d) => d.work_seconds), 1);

  return (
    <div className="flex items-end gap-2 h-24">
      {data.map((d) => {
        const h = Math.round((d.work_seconds / maxWork) * 100);
        return (
          <div key={d.date} className="flex-1 flex flex-col items-center gap-1">
            <div className="w-full flex flex-col justify-end" style={{ height: "80px" }}>
              <div
                className="w-full rounded-t bg-green-400 transition-all"
                style={{ height: `${h}%`, minHeight: d.work_seconds > 0 ? "4px" : "0" }}
                title={fmtTime(d.work_seconds)}
              />
            </div>
            <span className="text-xs text-gray-400">{getLabel(d.date)}</span>
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Stat card
// ---------------------------------------------------------------------------

function StatCard({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="bg-gray-50 rounded-xl p-4">
      <p className="text-xs text-gray-500 font-medium uppercase tracking-wide">{label}</p>
      <p className="text-2xl font-semibold text-gray-900 mt-1">{value}</p>
      {sub && <p className="text-xs text-gray-400 mt-0.5">{sub}</p>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

export default function StatsPage() {
  const [data, setData] = useState<DayStat[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const range = getLast7Days();
    getStats(range)
      .then(setData)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  const totalWork = data.reduce((s, d) => s + d.work_seconds, 0);
  const totalBreaks = data.reduce((s, d) => s + d.break_count, 0);
  const totalSkips = data.reduce((s, d) => s + d.skip_count, 0);
  const skipRate =
    totalBreaks + totalSkips > 0
      ? Math.round((totalSkips / (totalBreaks + totalSkips)) * 100)
      : 0;

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-gray-900">Statistics</h2>
        <p className="text-sm text-gray-500 mt-1">Last 7 days overview</p>
      </div>

      {loading && <p className="text-sm text-gray-400 text-center py-8">Loading...</p>}

      {error && (
        <p className="text-sm text-red-500 bg-red-50 rounded-lg px-3 py-2">{error}</p>
      )}

      {!loading && !error && (
        <>
          <div className="grid grid-cols-3 gap-3">
            <StatCard label="Work time" value={fmtTime(totalWork)} sub="total" />
            <StatCard label="Breaks taken" value={String(totalBreaks)} sub="completed" />
            <StatCard label="Skip rate" value={`${skipRate}%`} sub={`${totalSkips} skipped`} />
          </div>

          {data.length > 0 ? (
            <div>
              <p className="text-xs font-semibold uppercase tracking-wider text-gray-400 mb-3">
                Work time per day
              </p>
              <BarChart data={data} />
            </div>
          ) : (
            <p className="text-sm text-gray-400 text-center py-8">
              No data for this period yet.
            </p>
          )}
        </>
      )}
    </div>
  );
}
