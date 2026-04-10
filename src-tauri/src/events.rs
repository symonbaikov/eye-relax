use tokio::sync::broadcast;

use crate::config::AppConfig;

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakType {
    Short,
    Long,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerState {
    Idle,
    Working,
    OnBreak,
    Paused,
}

/// All events that flow through the application bus.
#[derive(Debug, Clone)]
pub enum AppEvent {
    // Scheduler
    PreBreakPromptTick {
        break_type: BreakType,
        remaining_secs: u64,
    },
    PreBreakPromptHidden,
    BreakDue {
        break_type: BreakType,
    },
    BreakCompleted,
    BreakSkipped,
    BreakDeferred {
        secs: u64,
    },
    BreakSnoozed {
        secs: u64,
    },
    StateChanged(SchedulerState),

    // Activity
    UserIdle {
        idle_secs: u64,
    },
    UserReturned,

    // Config
    ConfigUpdated(AppConfig),

    // Break countdown tick (sent every second during OnBreak)
    BreakTick {
        remaining_secs: u64,
    },

    // Stats
    SessionTick {
        work_secs: u64,
    },
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

const BUS_CAPACITY: usize = 64;

/// Central broadcast bus. Clone-safe handle — cheap to pass around.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BUS_CAPACITY);
        EventBus { tx }
    }

    /// Emit an event. Non-blocking. If the buffer is full the event is dropped
    /// with a warning (lagged receivers are expected to handle `RecvError::Lagged`).
    pub fn emit(&self, event: AppEvent) {
        match self.tx.send(event) {
            Ok(_) => {}
            Err(_) => {
                // No active receivers — this is fine during startup/shutdown.
                tracing::trace!("EventBus: no receivers, event dropped");
            }
        }
    }

    /// Create a new receiver. Each receiver gets an independent queue.
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn eventbus_emit_received_by_subscriber() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit(AppEvent::BreakCompleted);

        let event = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        assert!(matches!(event, AppEvent::BreakCompleted));
    }

    #[tokio::test]
    async fn eventbus_two_subscribers_both_receive() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.emit(AppEvent::UserReturned);

        let e1 = timeout(Duration::from_millis(100), rx1.recv())
            .await
            .expect("timed out")
            .expect("closed");
        let e2 = timeout(Duration::from_millis(100), rx2.recv())
            .await
            .expect("timed out")
            .expect("closed");

        assert!(matches!(e1, AppEvent::UserReturned));
        assert!(matches!(e2, AppEvent::UserReturned));
    }

    #[tokio::test]
    async fn eventbus_buffer_overflow_drops_event_no_panic() {
        let bus = EventBus::new();
        // Subscribe but never read — fills the buffer.
        let _rx = bus.subscribe();

        // Emit more events than the buffer capacity: must not panic.
        for i in 0..(super::BUS_CAPACITY + 10) {
            bus.emit(AppEvent::SessionTick {
                work_secs: i as u64,
            });
        }
        // If we reach here without panic, the test passes.
    }

    #[tokio::test]
    async fn eventbus_emit_without_receivers_no_panic() {
        let bus = EventBus::new();
        // No subscribers at all.
        bus.emit(AppEvent::BreakCompleted);
        bus.emit(AppEvent::UserIdle { idle_secs: 300 });
    }
}
