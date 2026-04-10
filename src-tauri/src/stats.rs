use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::events::{AppEvent, BreakType, EventBus, SchedulerState};
use crate::storage::{BreakRecord, Session, StoragePort};

const FLUSH_INTERVAL: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct StatsInner {
    session: Session,
    working_since: Option<std::time::Instant>,
    current_break_id: Option<String>,
    current_break_started: Option<String>,
    current_break_type: Option<BreakType>,
}

impl StatsInner {
    fn new(session: Session) -> Self {
        Self {
            session,
            working_since: None,
            current_break_id: None,
            current_break_started: None,
            current_break_type: None,
        }
    }

    fn start_working(&mut self) {
        if self.working_since.is_none() {
            self.working_since = Some(std::time::Instant::now());
        }
    }

    fn stop_working(&mut self) {
        if let Some(since) = self.working_since.take() {
            self.session.work_seconds += since.elapsed().as_secs();
        }
    }

    fn start_break(&mut self, break_type: BreakType) {
        self.stop_working();
        self.current_break_id = Some(Uuid::new_v4().to_string());
        self.current_break_started = Some(chrono::Utc::now().to_rfc3339());
        self.current_break_type = Some(break_type);
    }

    fn finish_break(&mut self, status: &str) -> Option<BreakRecord> {
        let id = self.current_break_id.take()?;
        let started_at = self.current_break_started.take()?;
        let break_type = self.current_break_type.take()?;

        let type_str = match break_type {
            BreakType::Short => "short",
            BreakType::Long => "long",
        };

        match status {
            "completed" => self.session.break_count += 1,
            "skipped" | "snoozed" => self.session.skip_count += 1,
            _ => {}
        }

        Some(BreakRecord {
            id,
            break_type: type_str.to_string(),
            status: status.to_string(),
            started_at,
            ended_at: Some(chrono::Utc::now().to_rfc3339()),
        })
    }
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Subscribes to the EventBus, tracks work time and break events, and
/// periodically flushes the current session to storage.
pub fn spawn_stats_aggregator(storage: Arc<dyn StoragePort>, bus: Arc<EventBus>) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let (session_id, initial_work, initial_breaks, initial_skips) =
        match storage.get_today_session(&today) {
            Ok(Some(s)) => (s.id, s.work_seconds, s.break_count, s.skip_count),
            _ => (Uuid::new_v4().to_string(), 0, 0, 0),
        };

    let session = Session {
        id: session_id,
        date: today,
        work_seconds: initial_work,
        break_count: initial_breaks,
        skip_count: initial_skips,
    };

    let inner = Arc::new(Mutex::new(StatsInner::new(session)));

    // ── EventBus listener ────────────────────────────────────────────────────
    let inner_ev = Arc::clone(&inner);
    let storage_ev = Arc::clone(&storage);
    let mut rx = bus.subscribe();

    crate::spawn_async(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::StateChanged(state)) => {
                    let mut g = inner_ev.lock().unwrap();
                    match state {
                        SchedulerState::Working => g.start_working(),
                        _ => g.stop_working(),
                    }
                }
                Ok(AppEvent::BreakDue { break_type }) => {
                    inner_ev.lock().unwrap().start_break(break_type);
                }
                Ok(AppEvent::BreakCompleted) => {
                    let record = inner_ev.lock().unwrap().finish_break("completed");
                    if let Some(r) = record {
                        if let Err(e) = storage_ev.record_break(&r) {
                            tracing::error!("Stats: failed to record break: {e}");
                        }
                    }
                }
                Ok(AppEvent::BreakSkipped) => {
                    let record = inner_ev.lock().unwrap().finish_break("skipped");
                    if let Some(r) = record {
                        if let Err(e) = storage_ev.record_break(&r) {
                            tracing::error!("Stats: failed to record break: {e}");
                        }
                    }
                }
                Ok(AppEvent::BreakSnoozed { .. }) => {
                    let record = inner_ev.lock().unwrap().finish_break("snoozed");
                    if let Some(r) = record {
                        if let Err(e) = storage_ev.record_break(&r) {
                            tracing::error!("Stats: failed to record break: {e}");
                        }
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Stats listener lagged {n} events");
                }
                _ => {}
            }
        }
    });

    // ── Periodic flush ────────────────────────────────────────────────────────
    crate::spawn_async(async move {
        let mut interval = tokio::time::interval(FLUSH_INTERVAL);
        loop {
            interval.tick().await;

            // Snapshot current accumulated work time (including ongoing work).
            let session = {
                let mut g = inner.lock().unwrap();
                // Credit any ongoing work period.
                if let Some(since) = g.working_since {
                    g.session.work_seconds += since.elapsed().as_secs();
                    g.working_since = Some(std::time::Instant::now()); // reset base
                }
                g.session.clone()
            };

            if let Err(e) = storage.upsert_session(&session) {
                tracing::error!("Stats flush failed: {e}");
            } else {
                tracing::debug!(
                    "Stats flushed: {} work_secs, {} breaks, {} skips",
                    session.work_seconds,
                    session.break_count,
                    session.skip_count,
                );
            }
        }
    });
}
