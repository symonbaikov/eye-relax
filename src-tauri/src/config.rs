use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::storage::StoragePort;

// ---------------------------------------------------------------------------
// AppConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Seconds of work before a short break. Range: 300–3600 (5–60 min).
    pub work_interval_secs: u64,
    /// Duration of a short break in seconds. Range: 10–60.
    pub break_duration_secs: u64,
    /// Seconds of work before a long break. Range: 1800–7200 (30–120 min).
    pub long_break_interval_secs: u64,
    /// Duration of a long break in seconds. Range: 120–900 (2–15 min).
    pub long_break_duration_secs: u64,
    /// Snooze duration in seconds. Range: 60–600 (1–10 min).
    pub snooze_duration_secs: u64,
    /// Idle threshold in seconds before timer resets. Range: 120–900 (2–15 min).
    pub idle_threshold_secs: u64,
    /// Play a sound when a break starts.
    pub sound_enabled: bool,
    /// Launch at login.
    pub autostart: bool,
    /// UI colour theme.
    pub theme: Theme,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            work_interval_secs: 1200,       // 20 min
            break_duration_secs: 20,
            long_break_interval_secs: 3600, // 60 min
            long_break_duration_secs: 300,  // 5 min
            snooze_duration_secs: 300,      // 5 min
            idle_threshold_secs: 300,       // 5 min
            sound_enabled: true,
            autostart: true,
            theme: Theme::System,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("work_interval_secs must be between 300 and 3600, got {0}")]
    WorkInterval(u64),
    #[error("break_duration_secs must be between 10 and 60, got {0}")]
    BreakDuration(u64),
    #[error("long_break_interval_secs must be between 1800 and 7200, got {0}")]
    LongBreakInterval(u64),
    #[error("long_break_duration_secs must be between 120 and 900, got {0}")]
    LongBreakDuration(u64),
    #[error("snooze_duration_secs must be between 60 and 600, got {0}")]
    SnoozeDuration(u64),
    #[error("idle_threshold_secs must be between 120 and 900, got {0}")]
    IdleThreshold(u64),
    #[error("Storage error: {0}")]
    Storage(#[from] crate::storage::StorageError),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

impl AppConfig {
    /// Validate all fields. Returns `Err` describing the first violation.
    pub fn validate(&self) -> Result<()> {
        if !(300..=3600).contains(&self.work_interval_secs) {
            return Err(ConfigError::WorkInterval(self.work_interval_secs));
        }
        if !(10..=60).contains(&self.break_duration_secs) {
            return Err(ConfigError::BreakDuration(self.break_duration_secs));
        }
        if !(1800..=7200).contains(&self.long_break_interval_secs) {
            return Err(ConfigError::LongBreakInterval(self.long_break_interval_secs));
        }
        if !(120..=900).contains(&self.long_break_duration_secs) {
            return Err(ConfigError::LongBreakDuration(self.long_break_duration_secs));
        }
        if !(60..=600).contains(&self.snooze_duration_secs) {
            return Err(ConfigError::SnoozeDuration(self.snooze_duration_secs));
        }
        if !(120..=900).contains(&self.idle_threshold_secs) {
            return Err(ConfigError::IdleThreshold(self.idle_threshold_secs));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ConfigManager
// ---------------------------------------------------------------------------

/// Loads config from storage on startup, caches it in memory, validates on save.
pub struct ConfigManager {
    storage: Arc<dyn StoragePort>,
    current: RwLock<AppConfig>,
}

impl ConfigManager {
    pub fn new(storage: Arc<dyn StoragePort>) -> Self {
        let config = storage.load_config().unwrap_or_else(|err| {
            tracing::warn!("Failed to load config, using defaults: {err}");
            AppConfig::default()
        });
        tracing::info!("Config loaded");
        ConfigManager {
            storage,
            current: RwLock::new(config),
        }
    }

    /// Returns a snapshot of the current config.
    pub fn current(&self) -> AppConfig {
        self.current.read().unwrap().clone()
    }

    /// Validate and persist new config. On success the in-memory cache is updated.
    pub fn update(&self, config: AppConfig) -> Result<()> {
        config.validate()?;
        self.storage.save_config(&config)?;
        *self.current.write().unwrap() = config;
        tracing::info!("Config updated and saved");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::mock::MockStorage;

    fn valid() -> AppConfig {
        AppConfig::default()
    }

    // --- work_interval_secs ---

    #[test]
    fn config_work_interval_min_valid() {
        let mut c = valid();
        c.work_interval_secs = 300;
        assert!(c.validate().is_ok());
    }

    #[test]
    fn config_work_interval_max_valid() {
        let mut c = valid();
        c.work_interval_secs = 3600;
        assert!(c.validate().is_ok());
    }

    #[test]
    fn config_work_interval_below_min_rejected() {
        let mut c = valid();
        c.work_interval_secs = 299;
        assert!(matches!(c.validate(), Err(ConfigError::WorkInterval(299))));
    }

    #[test]
    fn config_work_interval_above_max_rejected() {
        let mut c = valid();
        c.work_interval_secs = 3601;
        assert!(matches!(c.validate(), Err(ConfigError::WorkInterval(3601))));
    }

    // --- break_duration_secs ---

    #[test]
    fn config_break_duration_min_valid() {
        let mut c = valid();
        c.break_duration_secs = 10;
        assert!(c.validate().is_ok());
    }

    #[test]
    fn config_break_duration_max_valid() {
        let mut c = valid();
        c.break_duration_secs = 60;
        assert!(c.validate().is_ok());
    }

    #[test]
    fn config_break_duration_below_min_rejected() {
        let mut c = valid();
        c.break_duration_secs = 9;
        assert!(matches!(c.validate(), Err(ConfigError::BreakDuration(9))));
    }

    #[test]
    fn config_break_duration_above_max_rejected() {
        let mut c = valid();
        c.break_duration_secs = 61;
        assert!(matches!(c.validate(), Err(ConfigError::BreakDuration(61))));
    }

    // --- long_break_interval_secs ---

    #[test]
    fn config_long_break_interval_below_min_rejected() {
        let mut c = valid();
        c.long_break_interval_secs = 1799;
        assert!(matches!(
            c.validate(),
            Err(ConfigError::LongBreakInterval(1799))
        ));
    }

    #[test]
    fn config_long_break_interval_above_max_rejected() {
        let mut c = valid();
        c.long_break_interval_secs = 7201;
        assert!(matches!(
            c.validate(),
            Err(ConfigError::LongBreakInterval(7201))
        ));
    }

    // --- long_break_duration_secs ---

    #[test]
    fn config_long_break_duration_below_min_rejected() {
        let mut c = valid();
        c.long_break_duration_secs = 119;
        assert!(matches!(
            c.validate(),
            Err(ConfigError::LongBreakDuration(119))
        ));
    }

    #[test]
    fn config_long_break_duration_above_max_rejected() {
        let mut c = valid();
        c.long_break_duration_secs = 901;
        assert!(matches!(
            c.validate(),
            Err(ConfigError::LongBreakDuration(901))
        ));
    }

    // --- snooze_duration_secs ---

    #[test]
    fn config_snooze_below_min_rejected() {
        let mut c = valid();
        c.snooze_duration_secs = 59;
        assert!(matches!(c.validate(), Err(ConfigError::SnoozeDuration(59))));
    }

    #[test]
    fn config_snooze_above_max_rejected() {
        let mut c = valid();
        c.snooze_duration_secs = 601;
        assert!(matches!(c.validate(), Err(ConfigError::SnoozeDuration(601))));
    }

    // --- idle_threshold_secs ---

    #[test]
    fn config_idle_threshold_below_min_rejected() {
        let mut c = valid();
        c.idle_threshold_secs = 119;
        assert!(matches!(c.validate(), Err(ConfigError::IdleThreshold(119))));
    }

    #[test]
    fn config_idle_threshold_above_max_rejected() {
        let mut c = valid();
        c.idle_threshold_secs = 901;
        assert!(matches!(c.validate(), Err(ConfigError::IdleThreshold(901))));
    }

    // --- ConfigManager ---

    #[test]
    fn config_manager_invalid_rejected() {
        let storage = Arc::new(MockStorage::new());
        let manager = ConfigManager::new(storage);
        let mut bad = valid();
        bad.work_interval_secs = 1; // out of range
        assert!(manager.update(bad).is_err());
    }

    #[test]
    fn config_manager_save_idempotent() {
        let storage = Arc::new(MockStorage::new());
        let manager = ConfigManager::new(Arc::clone(&storage) as Arc<dyn StoragePort>);
        let cfg = valid();
        manager.update(cfg.clone()).unwrap();
        manager.update(cfg.clone()).unwrap();
        assert_eq!(*storage.save_count.lock().unwrap(), 2);
        assert_eq!(manager.current(), cfg);
    }
}
