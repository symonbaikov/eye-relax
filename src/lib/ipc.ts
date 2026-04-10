import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type Theme = "light" | "dark" | "system";

export interface AppConfig {
  work_interval_secs: number;
  break_duration_secs: number;
  long_break_interval_secs: number;
  long_break_duration_secs: number;
  snooze_duration_secs: number;
  idle_threshold_secs: number;
  sound_enabled: boolean;
  autostart: boolean;
  theme: Theme;
}

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

export const getConfig = (): Promise<AppConfig> => invoke<AppConfig>("get_config");

export const setConfig = (config: AppConfig): Promise<void> =>
  invoke<void>("set_config", { config });

// ---------------------------------------------------------------------------
// Scheduler commands
// ---------------------------------------------------------------------------

export type SchedulerState = "idle" | "working" | "on_break" | "paused";

export const getState = (): Promise<SchedulerState> =>
  invoke<SchedulerState>("get_state");

export const getRemaining = (): Promise<number> =>
  invoke<number>("get_remaining");

export const skipBreak = (): Promise<void> => invoke<void>("skip_break");

export const snoozeBreak = (durationSecs: number): Promise<void> =>
  invoke<void>("snooze_break", { duration_secs: durationSecs });

export const pauseTimer = (): Promise<void> => invoke<void>("pause_timer");

export const resumeTimer = (): Promise<void> => invoke<void>("resume_timer");

// ---------------------------------------------------------------------------
// Stats commands
// ---------------------------------------------------------------------------

export interface DateRange {
  start: string; // YYYY-MM-DD
  end: string;   // YYYY-MM-DD
}

export interface DayStat {
  date: string;
  work_seconds: number;
  break_count: number;
  skip_count: number;
}

export const getStats = (range: DateRange): Promise<DayStat[]> =>
  invoke<DayStat[]>("get_stats", { range });
