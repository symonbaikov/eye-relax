use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::AppConfig;
use crate::events::{AppEvent, EventBus};
use crate::platform::ActivitySource;

// ---------------------------------------------------------------------------
// ActivityTracker
// ---------------------------------------------------------------------------

/// Polls `ActivitySource` every `POLL_INTERVAL` seconds.
/// Emits `UserIdle` when idle time exceeds the configured threshold,
/// and `UserReturned` when the user comes back.
pub struct ActivityTracker {
    source: Arc<dyn ActivitySource>,
    bus: Arc<EventBus>,
    config: Arc<Mutex<AppConfig>>,
}

const POLL_INTERVAL: Duration = Duration::from_secs(30);

impl ActivityTracker {
    pub fn new(
        source: Arc<dyn ActivitySource>,
        bus: Arc<EventBus>,
        config: AppConfig,
    ) -> Arc<Self> {
        let tracker = Arc::new(ActivityTracker {
            source,
            bus,
            config: Arc::new(Mutex::new(config)),
        });
        tracker.spawn_poll_loop();
        tracker
    }

    /// Update the config (called when ConfigUpdated event arrives).
    pub fn update_config(&self, config: AppConfig) {
        *self.config.lock().unwrap() = config;
    }

    fn spawn_poll_loop(&self) {
        let source = Arc::clone(&self.source);
        let bus = Arc::clone(&self.bus);
        let config = Arc::clone(&self.config);

        crate::spawn_async(async move {
            let mut was_idle = false;

            loop {
                tokio::time::sleep(POLL_INTERVAL).await;

                let idle_secs = match source.idle_seconds() {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("ActivityTracker: failed to query idle: {e}");
                        continue;
                    }
                };

                let threshold = config.lock().unwrap().idle_threshold_secs;

                if idle_secs >= threshold && !was_idle {
                    tracing::info!("User idle ({idle_secs}s ≥ {threshold}s threshold)");
                    was_idle = true;
                    bus.emit(AppEvent::UserIdle { idle_secs });
                } else if idle_secs < threshold && was_idle {
                    tracing::info!("User returned (idle {idle_secs}s < {threshold}s threshold)");
                    was_idle = false;
                    bus.emit(AppEvent::UserReturned);
                } else {
                    tracing::trace!("Activity poll: idle={idle_secs}s, threshold={threshold}s");
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Mock (for tests)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod mock {
    use std::sync::Mutex;

    use super::super::platform::ActivitySource;

    pub struct MockActivity {
        pub idle_secs: Mutex<u64>,
        pub locked: Mutex<bool>,
    }

    impl MockActivity {
        pub fn new(idle_secs: u64) -> Self {
            MockActivity {
                idle_secs: Mutex::new(idle_secs),
                locked: Mutex::new(false),
            }
        }

        pub fn set_idle(&self, secs: u64) {
            *self.idle_secs.lock().unwrap() = secs;
        }
    }

    impl ActivitySource for MockActivity {
        fn idle_seconds(&self) -> anyhow::Result<u64> {
            Ok(*self.idle_secs.lock().unwrap())
        }

        fn is_screen_locked(&self) -> anyhow::Result<bool> {
            Ok(*self.locked.lock().unwrap())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::time::{self, Duration};

    use super::mock::MockActivity;
    use super::*;
    use crate::events::AppEvent;

    fn make(idle_secs: u64, threshold_secs: u64) -> (Arc<ActivityTracker>, Arc<EventBus>) {
        let mut cfg = AppConfig::default();
        cfg.idle_threshold_secs = threshold_secs;
        let source = Arc::new(MockActivity::new(idle_secs));
        let bus = Arc::new(EventBus::new());
        let tracker = ActivityTracker::new(source, Arc::clone(&bus), cfg);
        (tracker, bus)
    }

    #[tokio::test]
    async fn tracker_idle_above_threshold_emits_user_idle() {
        time::pause();
        // idle = 400s, threshold = 300s → should emit UserIdle
        let (_tracker, bus) = make(400, 300);
        let mut rx = bus.subscribe();

        // Advance past the poll interval
        tokio::task::yield_now().await;
        time::advance(Duration::from_secs(31)).await;
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }

        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, AppEvent::UserIdle { idle_secs: 400 }),
            "expected UserIdle(400), got {event:?}"
        );
    }

    #[tokio::test]
    async fn tracker_idle_below_threshold_no_event() {
        time::pause();
        // idle = 100s, threshold = 300s → should NOT emit
        let (_tracker, bus) = make(100, 300);
        let mut rx = bus.subscribe();

        tokio::task::yield_now().await;
        time::advance(Duration::from_secs(31)).await;
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }

        // Channel should be empty — try_recv returns empty
        match rx.try_recv() {
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {}
            other => panic!("expected no event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn tracker_user_returned_after_idle() {
        time::pause();
        let mut cfg = AppConfig::default();
        cfg.idle_threshold_secs = 300;
        let source = Arc::new(MockActivity::new(400)); // starts idle
        let bus = Arc::new(EventBus::new());
        let _tracker = ActivityTracker::new(Arc::clone(&source) as Arc<dyn ActivitySource>, Arc::clone(&bus), cfg);
        let mut rx = bus.subscribe();

        // First poll → UserIdle
        tokio::task::yield_now().await;
        time::advance(Duration::from_secs(31)).await;
        for _ in 0..4 { tokio::task::yield_now().await; }

        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::UserIdle { .. }));

        // User comes back
        source.set_idle(5);

        // Second poll → UserReturned
        time::advance(Duration::from_secs(31)).await;
        for _ in 0..4 { tokio::task::yield_now().await; }

        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::UserReturned));
    }

    #[tokio::test]
    async fn tracker_no_duplicate_idle_events() {
        time::pause();
        let (_tracker, bus) = make(400, 300);
        let mut rx = bus.subscribe();

        // Two consecutive polls while still idle → only one UserIdle event
        for _ in 0..2 {
            tokio::task::yield_now().await;
            time::advance(Duration::from_secs(31)).await;
            for _ in 0..4 { tokio::task::yield_now().await; }
        }

        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::UserIdle { .. }));

        // Second receive should be empty
        match rx.try_recv() {
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {}
            other => panic!("expected no second event, got {other:?}"),
        }
    }
}
