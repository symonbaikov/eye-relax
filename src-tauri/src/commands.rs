use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use crate::config::{AppConfig, ConfigManager};
use crate::events::SchedulerState;
use crate::scheduler::{SchedulerPort, TimerScheduler};
use crate::storage::{DateRange, DayStat, StoragePort};

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
pub fn set_config(
    config: AppConfig,
    config_manager: State<Arc<ConfigManager>>,
) -> IpcResult<()> {
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
pub fn skip_break(scheduler: State<Arc<TimerScheduler>>) {
    scheduler.skip();
}

#[tauri::command]
pub fn snooze_break(duration_secs: u64, scheduler: State<Arc<TimerScheduler>>) {
    scheduler.snooze(Duration::from_secs(duration_secs));
}

#[tauri::command]
pub fn pause_timer(scheduler: State<Arc<TimerScheduler>>) {
    scheduler.pause();
}

#[tauri::command]
pub fn resume_timer(scheduler: State<Arc<TimerScheduler>>) {
    scheduler.resume();
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
