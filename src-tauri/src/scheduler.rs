use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::AppConfig;
use crate::events::{AppEvent, BreakType, EventBus, SchedulerState};

// ---------------------------------------------------------------------------
// Port (trait)
// ---------------------------------------------------------------------------

/// External interface to the scheduler. All methods are synchronous and
/// non-blocking — they mutate state and (re)spawn background tasks.
pub trait SchedulerPort: Send + Sync {
    fn start(&self);
    fn force_break(&self);
    fn skip(&self);
    fn snooze(&self, duration: Duration);
    fn pause(&self);
    fn resume(&self);
    fn state(&self) -> SchedulerState;
    fn remaining_secs(&self) -> u64;
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct SchedulerInner {
    state: SchedulerState,
    /// Seconds remaining in the current phase (work or break).
    remaining_secs: u64,
    /// Completed work intervals — used to decide when to trigger a long break.
    work_cycles: u32,
    config: AppConfig,
    /// Cancel flag for the currently running timer task.
    /// Set to `true` to signal the task to stop at the next sleep boundary.
    task: Option<Arc<AtomicBool>>,
}

impl SchedulerInner {
    fn new(config: AppConfig) -> Self {
        SchedulerInner {
            state: SchedulerState::Idle,
            remaining_secs: config.work_interval_secs,
            work_cycles: 0,
            config,
            task: None,
        }
    }

    fn next_break_type(&self) -> BreakType {
        let cycles_per_long =
            (self.config.long_break_interval_secs / self.config.work_interval_secs).max(1);
        if (self.work_cycles + 1).is_multiple_of(cycles_per_long as u32) {
            BreakType::Long
        } else {
            BreakType::Short
        }
    }

    fn break_duration(&self, bt: &BreakType) -> u64 {
        match bt {
            BreakType::Short => self.config.break_duration_secs,
            BreakType::Long => self.config.long_break_duration_secs,
        }
    }

    fn abort_task(&mut self) {
        if let Some(cancel) = self.task.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

// ---------------------------------------------------------------------------
// TimerScheduler
// ---------------------------------------------------------------------------

pub struct TimerScheduler {
    inner: Arc<Mutex<SchedulerInner>>,
    bus: Arc<EventBus>,
}

impl TimerScheduler {
    pub fn new(bus: Arc<EventBus>, config: AppConfig) -> Arc<Self> {
        let scheduler = Arc::new(TimerScheduler {
            inner: Arc::new(Mutex::new(SchedulerInner::new(config))),
            bus: Arc::clone(&bus),
        });

        // Subscribe to bus events (UserIdle, UserReturned, ConfigUpdated).
        scheduler.spawn_bus_listener(bus);

        scheduler
    }

    fn spawn_bus_listener(&self, bus: Arc<EventBus>) {
        let inner = Arc::clone(&self.inner);
        let bus_for_task = Arc::clone(&bus);
        let mut rx = bus.subscribe();

        crate::spawn_async(async move {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::UserIdle { .. }) => {
                        let mut g = inner.lock().unwrap();
                        if g.state == SchedulerState::Working {
                            g.abort_task();
                            g.state = SchedulerState::Idle;
                            g.remaining_secs = g.config.work_interval_secs;
                            tracing::info!("State transition: Working → Idle (user idle)");
                            bus_for_task.emit(AppEvent::StateChanged(SchedulerState::Idle));
                        }
                    }
                    Ok(AppEvent::UserReturned) => {
                        let should_start = {
                            let g = inner.lock().unwrap();
                            g.state == SchedulerState::Idle
                        };
                        if should_start {
                            Self::do_start_working(&inner, &bus_for_task);
                        }
                    }
                    Ok(AppEvent::ConfigUpdated(cfg)) => {
                        let mut g = inner.lock().unwrap();
                        g.config = cfg;
                        // If currently working, restart the timer with the new interval.
                        if g.state == SchedulerState::Working {
                            g.abort_task();
                            let remaining = g.config.work_interval_secs;
                            g.remaining_secs = remaining;
                            drop(g);
                            Self::spawn_work_task(&inner, &bus_for_task, remaining);
                            tracing::info!("Config updated — work timer restarted");
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Scheduler bus listener lagged by {n} events");
                    }
                    _ => {}
                }
            }
        });
    }

    // -----------------------------------------------------------------------
    // Helpers used from both the scheduler methods and the bus listener
    // -----------------------------------------------------------------------

    fn do_start_break(inner: &Arc<Mutex<SchedulerInner>>, bus: &Arc<EventBus>) {
        let (break_type, break_secs) = {
            let mut g = inner.lock().unwrap();
            g.abort_task();
            let bt = g.next_break_type();
            g.work_cycles += 1;
            let secs = g.break_duration(&bt);
            g.state = SchedulerState::OnBreak;
            g.remaining_secs = secs;
            (bt, secs)
        };
        tracing::info!("State transition: → OnBreak (forced, {break_type:?})");
        bus.emit(AppEvent::StateChanged(SchedulerState::OnBreak));
        bus.emit(AppEvent::BreakDue { break_type });
        Self::spawn_break_task(inner, bus, break_secs);
    }

    fn spawn_break_task(
        inner: &Arc<Mutex<SchedulerInner>>,
        bus: &Arc<EventBus>,
        break_secs: u64,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_t = Arc::clone(&cancel);
        let inner_t = Arc::clone(inner);
        let bus_t = Arc::clone(bus);

        crate::spawn_async(async move {
            for remaining in (0..break_secs).rev() {
                if cancel_t.load(Ordering::Relaxed) {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                inner_t.lock().unwrap().remaining_secs = remaining;
                bus_t.emit(AppEvent::BreakTick { remaining_secs: remaining });
            }
            if cancel_t.load(Ordering::Relaxed) {
                return;
            }

            let next_work_secs = {
                let mut g = inner_t.lock().unwrap();
                g.state = SchedulerState::Working;
                let r = g.config.work_interval_secs;
                g.remaining_secs = r;
                r
            };

            tracing::info!("State transition: OnBreak → Working (break completed)");
            bus_t.emit(AppEvent::BreakCompleted);
            bus_t.emit(AppEvent::StateChanged(SchedulerState::Working));
            Self::spawn_work_task(&inner_t, &bus_t, next_work_secs);
        });

        inner.lock().unwrap().task = Some(cancel);
    }

    fn do_start_working(inner: &Arc<Mutex<SchedulerInner>>, bus: &Arc<EventBus>) {
        let remaining = {
            let mut g = inner.lock().unwrap();
            g.abort_task();
            g.state = SchedulerState::Working;
            let r = g.config.work_interval_secs;
            g.remaining_secs = r;
            r
        };
        tracing::info!("State transition: → Working");
        bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(inner, bus, remaining);
    }

    fn spawn_work_task(
        inner: &Arc<Mutex<SchedulerInner>>,
        bus: &Arc<EventBus>,
        work_secs: u64,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_t = Arc::clone(&cancel);
        let inner_t = Arc::clone(inner);
        let bus_t = Arc::clone(bus);

        crate::spawn_async(async move {
            // --- Work countdown ---
            for remaining in (0..work_secs).rev() {
                if cancel_t.load(Ordering::Relaxed) {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                inner_t.lock().unwrap().remaining_secs = remaining;
            }
            if cancel_t.load(Ordering::Relaxed) {
                return;
            }

            // --- Transition to break ---
            let (break_type, break_secs) = {
                let mut g = inner_t.lock().unwrap();
                let bt = g.next_break_type();
                g.work_cycles += 1;
                let secs = g.break_duration(&bt);
                g.state = SchedulerState::OnBreak;
                g.remaining_secs = secs;
                (bt, secs)
            };

            tracing::info!("State transition: Working → OnBreak ({break_type:?})");
            bus_t.emit(AppEvent::StateChanged(SchedulerState::OnBreak));
            bus_t.emit(AppEvent::BreakDue { break_type });
            Self::spawn_break_task(&inner_t, &bus_t, break_secs);
        });

        inner.lock().unwrap().task = Some(cancel);
    }
}

impl SchedulerPort for TimerScheduler {
    /// Start the scheduler. No-op if already Working.
    fn start(&self) {
        let current = self.inner.lock().unwrap().state.clone();
        if current != SchedulerState::Idle {
            tracing::debug!("start() called in state {current:?} — no-op");
            return;
        }
        tracing::info!("State transition: Idle → Working");
        Self::do_start_working(&self.inner, &self.bus);
    }

    /// Force an immediate break regardless of current state.
    fn force_break(&self) {
        let current = self.inner.lock().unwrap().state.clone();
        if current == SchedulerState::OnBreak {
            tracing::debug!("force_break() called while already OnBreak — no-op");
            return;
        }
        tracing::info!("Force break requested");
        Self::do_start_break(&self.inner, &self.bus);
    }

    /// Skip the current break. No-op if not OnBreak.
    fn skip(&self) {
        let current = self.inner.lock().unwrap().state.clone();
        if current != SchedulerState::OnBreak {
            tracing::debug!("skip() called in state {current:?} — no-op");
            return;
        }
        tracing::info!("State transition: OnBreak → Working (skipped)");
        self.bus.emit(AppEvent::BreakSkipped);
        Self::do_start_working(&self.inner, &self.bus);
    }

    /// Snooze the current break. No-op if not OnBreak.
    fn snooze(&self, duration: Duration) {
        let current = self.inner.lock().unwrap().state.clone();
        if current != SchedulerState::OnBreak {
            tracing::debug!("snooze() called in state {current:?} — no-op");
            return;
        }
        let secs = duration.as_secs();
        tracing::info!("State transition: OnBreak → Working (snoozed {secs}s)");
        self.bus.emit(AppEvent::BreakSnoozed { secs });

        {
            let mut g = self.inner.lock().unwrap();
            g.abort_task();
            g.state = SchedulerState::Working;
            g.remaining_secs = secs;
        }
        self.bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(&self.inner, &self.bus, secs);
    }

    /// Pause the scheduler. No-op if not Working.
    fn pause(&self) {
        let current = self.inner.lock().unwrap().state.clone();
        if current != SchedulerState::Working {
            tracing::debug!("pause() called in state {current:?} — no-op");
            return;
        }
        self.inner.lock().unwrap().abort_task();
        self.inner.lock().unwrap().state = SchedulerState::Paused;
        tracing::info!("State transition: Working → Paused");
        self.bus.emit(AppEvent::StateChanged(SchedulerState::Paused));
    }

    /// Resume from Paused. No-op if not Paused.
    fn resume(&self) {
        let (current, remaining) = {
            let g = self.inner.lock().unwrap();
            (g.state.clone(), g.remaining_secs)
        };
        if current != SchedulerState::Paused {
            tracing::debug!("resume() called in state {current:?} — no-op");
            return;
        }
        self.inner.lock().unwrap().state = SchedulerState::Working;
        tracing::info!("State transition: Paused → Working");
        self.bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(&self.inner, &self.bus, remaining);
    }

    fn state(&self) -> SchedulerState {
        self.inner.lock().unwrap().state.clone()
    }

    fn remaining_secs(&self) -> u64 {
        self.inner.lock().unwrap().remaining_secs
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::AppEvent;
    use tokio::time::{self, Duration};

    fn test_config() -> AppConfig {
        AppConfig {
            work_interval_secs: 4,
            break_duration_secs: 2,
            long_break_interval_secs: 8,  // every 2 work cycles
            long_break_duration_secs: 4,
            snooze_duration_secs: 3,
            idle_threshold_secs: 300,
            ..AppConfig::default()
        }
    }

    fn make(cfg: AppConfig) -> (Arc<TimerScheduler>, Arc<EventBus>) {
        let bus = Arc::new(EventBus::new());
        let sched = TimerScheduler::new(Arc::clone(&bus), cfg);
        (sched, bus)
    }

    // --- FSM transitions ---

    #[tokio::test]
    async fn scheduler_idle_to_working_on_start() {
        let (sched, _bus) = make(test_config());
        assert_eq!(sched.state(), SchedulerState::Idle);
        sched.start();
        assert_eq!(sched.state(), SchedulerState::Working);
    }

    #[tokio::test]
    async fn scheduler_double_start_noop() {
        let (sched, _bus) = make(test_config());
        sched.start();
        let remaining_before = sched.remaining_secs();
        sched.start(); // must be no-op
        assert_eq!(sched.state(), SchedulerState::Working);
        assert_eq!(sched.remaining_secs(), remaining_before);
    }

    #[tokio::test]
    async fn scheduler_skip_noop_when_working() {
        let (sched, _bus) = make(test_config());
        sched.start();
        sched.skip(); // no-op: not OnBreak
        assert_eq!(sched.state(), SchedulerState::Working);
    }

    #[tokio::test]
    async fn scheduler_pause_from_working() {
        let (sched, _bus) = make(test_config());
        sched.start();
        sched.pause();
        assert_eq!(sched.state(), SchedulerState::Paused);
    }

    #[tokio::test]
    async fn scheduler_pause_noop_when_idle() {
        let (sched, _bus) = make(test_config());
        sched.pause(); // no-op: Idle
        assert_eq!(sched.state(), SchedulerState::Idle);
    }

    #[tokio::test]
    async fn scheduler_resume_from_paused() {
        let (sched, _bus) = make(test_config());
        sched.start();
        sched.pause();
        sched.resume();
        assert_eq!(sched.state(), SchedulerState::Working);
    }

    #[tokio::test]
    async fn scheduler_resume_noop_when_working() {
        let (sched, _bus) = make(test_config());
        sched.start();
        sched.resume(); // no-op
        assert_eq!(sched.state(), SchedulerState::Working);
    }

    #[tokio::test]
    async fn scheduler_pause_preserves_remaining() {
        time::pause();
        let cfg = test_config(); // work_interval_secs = 4
        let (sched, _bus) = make(cfg.clone());
        sched.start();

        // Advance past the full work interval so remaining definitely dropped.
        // Yield generously to let all spawned tasks (timer + bus listener) run.
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
        time::advance(Duration::from_secs(2)).await;
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }

        assert_eq!(sched.state(), SchedulerState::Working);
        let remaining_before_pause = sched.remaining_secs();
        // Some time was consumed, so remaining < initial work interval.
        assert!(
            remaining_before_pause < cfg.work_interval_secs,
            "expected remaining < {}, got {remaining_before_pause}",
            cfg.work_interval_secs
        );

        sched.pause();
        assert_eq!(sched.state(), SchedulerState::Paused);

        // Resume: remaining must be the same value as at pause time.
        sched.resume();
        assert_eq!(sched.state(), SchedulerState::Working);
        assert_eq!(sched.remaining_secs(), remaining_before_pause);
    }

    // --- Integration: full cycle ---

    #[tokio::test]
    async fn scheduler_full_cycle_idle_working_onbreak_working() {
        time::pause();
        let (sched, bus) = make(test_config());
        let mut rx = bus.subscribe();

        sched.start();

        // Consume StateChanged(Working)
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::StateChanged(SchedulerState::Working)));

        // Advance past the 4s work interval
        time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        // Expect StateChanged(OnBreak) + BreakDue
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::StateChanged(SchedulerState::OnBreak)));
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::BreakDue { break_type: BreakType::Short }));

        // Advance past the 2s break
        time::advance(Duration::from_secs(3)).await;
        tokio::task::yield_now().await;

        // Drain BreakTick events, find BreakCompleted
        loop {
            let ev = rx.recv().await.unwrap();
            if matches!(ev, AppEvent::BreakCompleted) {
                break;
            }
            assert!(matches!(ev, AppEvent::BreakTick { .. }));
        }

        // Should now be back to Working
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::StateChanged(SchedulerState::Working)));
        assert_eq!(sched.state(), SchedulerState::Working);
    }

    #[tokio::test]
    async fn scheduler_skip_during_break_returns_to_working() {
        time::pause();
        let (sched, bus) = make(test_config());
        let mut rx = bus.subscribe();

        sched.start();
        rx.recv().await.unwrap(); // StateChanged(Working)

        // Advance into break
        time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        rx.recv().await.unwrap(); // StateChanged(OnBreak)
        rx.recv().await.unwrap(); // BreakDue

        assert_eq!(sched.state(), SchedulerState::OnBreak);
        sched.skip();

        assert_eq!(sched.state(), SchedulerState::Working);

        // BreakSkipped + StateChanged(Working) should be on the bus
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::BreakSkipped));
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::StateChanged(SchedulerState::Working)));
    }

    #[tokio::test]
    async fn scheduler_snooze_during_break_delays_with_short_interval() {
        time::pause();
        let (sched, bus) = make(test_config());
        let mut rx = bus.subscribe();

        sched.start();
        rx.recv().await.unwrap(); // StateChanged(Working)

        time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        rx.recv().await.unwrap(); // StateChanged(OnBreak)
        rx.recv().await.unwrap(); // BreakDue

        sched.snooze(Duration::from_secs(3));

        assert_eq!(sched.state(), SchedulerState::Working);
        assert_eq!(sched.remaining_secs(), 3);

        // BreakSnoozed + StateChanged(Working) on the bus
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::BreakSnoozed { secs: 3 }));
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::StateChanged(SchedulerState::Working)));
    }

    #[tokio::test]
    async fn scheduler_user_idle_resets_working_to_idle() {
        time::pause();
        let (sched, bus) = make(test_config());
        let mut rx = bus.subscribe();

        sched.start();
        rx.recv().await.unwrap(); // StateChanged(Working)

        // Emit UserIdle — bus listener should reset to Idle
        bus.emit(AppEvent::UserIdle { idle_secs: 400 });
        // Yield so the bus listener task can process it
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        assert_eq!(sched.state(), SchedulerState::Idle);
    }

    #[tokio::test]
    async fn scheduler_long_break_after_n_cycles() {
        time::pause();
        let cfg = AppConfig {
            work_interval_secs: 2,
            break_duration_secs: 1,
            long_break_interval_secs: 4, // long break every 2 cycles
            long_break_duration_secs: 2,
            ..AppConfig::default()
        };
        let (sched, bus) = make(cfg);
        let mut rx = bus.subscribe();

        sched.start();
        rx.recv().await.unwrap(); // StateChanged(Working)

        // Cycle 1: short break
        time::advance(Duration::from_secs(3)).await;
        tokio::task::yield_now().await;
        rx.recv().await.unwrap(); // StateChanged(OnBreak)
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::BreakDue { break_type: BreakType::Short }));

        // Finish cycle 1 break
        time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;
        loop {
            let ev = rx.recv().await.unwrap();
            if matches!(ev, AppEvent::BreakCompleted) { break; }
        }
        rx.recv().await.unwrap(); // StateChanged(Working)

        // Cycle 2: long break
        time::advance(Duration::from_secs(3)).await;
        tokio::task::yield_now().await;
        rx.recv().await.unwrap(); // StateChanged(OnBreak)
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::BreakDue { break_type: BreakType::Long }));
    }
}
