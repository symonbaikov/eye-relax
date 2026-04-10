use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use crate::config::{AppConfig, ConfigManager};
use crate::events::SchedulerState;
use crate::power;
use crate::scheduler::{SchedulerPort, TimerScheduler};
use crate::screen_lock;
use crate::storage::{DateRange, DayStat, StoragePort};

const MAX_DAILY_SKIPS: u32 = 4;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipAllowance {
    pub used: u32,
    pub remaining: u32,
    pub limit: u32,
}

/// Tauri IPC error — serialized as a plain string for the frontend.
#[derive(Debug, serde::Serialize)]
pub struct IpcError(String);

impl<E: std::fmt::Display> From<E> for IpcError {
    fn from(e: E) -> Self {
        IpcError(e.to_string())
    }
}

type IpcResult<T> = std::result::Result<T, IpcError>;

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

/// Return the current application configuration.
#[tauri::command]
pub fn get_config(config_manager: State<Arc<ConfigManager>>) -> AppConfig {
    config_manager.current()
}

/// Validate and persist a new configuration.
#[tauri::command]
pub fn set_config(config: AppConfig, config_manager: State<Arc<ConfigManager>>) -> IpcResult<()> {
    config_manager.update(config).map_err(IpcError::from)
}

// ---------------------------------------------------------------------------
// Scheduler commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_state(scheduler: State<Arc<TimerScheduler>>) -> SchedulerState {
    scheduler.state()
}

#[tauri::command]
pub fn get_remaining(scheduler: State<Arc<TimerScheduler>>) -> u64 {
    scheduler.remaining_secs()
}

#[tauri::command]
pub fn skip_break(
    scheduler: State<Arc<TimerScheduler>>,
    storage: State<Arc<dyn StoragePort>>,
) -> IpcResult<()> {
    let allowance = get_skip_allowance_impl(storage.inner())?;
    if allowance.remaining == 0 {
        return Err(IpcError(format!(
            "Daily skip limit reached ({MAX_DAILY_SKIPS} per day)."
        )));
    }

    scheduler.skip();
    Ok(())
}

#[tauri::command]
pub fn snooze_break(duration_secs: u64, scheduler: State<Arc<TimerScheduler>>) {
    scheduler.snooze(Duration::from_secs(duration_secs));
}

#[tauri::command]
pub fn defer_break(duration_secs: u64, scheduler: State<Arc<TimerScheduler>>) {
    scheduler.defer_break(Duration::from_secs(duration_secs));
}

#[tauri::command]
pub fn pause_timer(scheduler: State<Arc<TimerScheduler>>) {
    scheduler.pause();
}

#[tauri::command]
pub fn resume_timer(scheduler: State<Arc<TimerScheduler>>) {
    scheduler.resume();
}

#[tauri::command]
pub fn lock_screen() -> IpcResult<()> {
    screen_lock::lock_screen().map_err(IpcError::from)
}

#[tauri::command]
pub fn suspend_system() -> IpcResult<()> {
    power::suspend_system().map_err(IpcError::from)
}

#[tauri::command]
pub fn get_skip_allowance(storage: State<Arc<dyn StoragePort>>) -> IpcResult<SkipAllowance> {
    get_skip_allowance_impl(storage.inner())
}

fn get_skip_allowance_impl(storage: &Arc<dyn StoragePort>) -> IpcResult<SkipAllowance> {
    let today = chrono::Local::now().date_naive();
    let start = today
        .and_hms_opt(0, 0, 0)
        .expect("valid start of day")
        .and_utc()
        .to_rfc3339();
    let end = today
        .succ_opt()
        .expect("next day exists")
        .and_hms_opt(0, 0, 0)
        .expect("valid end of day")
        .and_utc()
        .to_rfc3339();

    let used = storage
        .count_breaks_by_status(&start, &end, "skipped")
        .map_err(IpcError::from)?;
    let remaining = MAX_DAILY_SKIPS.saturating_sub(used);

    Ok(SkipAllowance {
        used,
        remaining,
        limit: MAX_DAILY_SKIPS,
    })
}

// ---------------------------------------------------------------------------
// Stats commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_stats(
    range: DateRange,
    storage: State<Arc<dyn StoragePort>>,
) -> IpcResult<Vec<DayStat>> {
    storage.get_stats(&range).map_err(IpcError::from)
}
