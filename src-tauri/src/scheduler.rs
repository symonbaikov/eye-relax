use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::AppConfig;
use crate::events::{AppEvent, BreakType, EventBus, SchedulerState};

const PRE_BREAK_PROMPT_SECS: u64 = 32;

// ---------------------------------------------------------------------------
// Port (trait)
// ---------------------------------------------------------------------------

/// External interface to the scheduler. All methods are synchronous and
/// non-blocking — they mutate state and (re)spawn background tasks.
pub trait SchedulerPort: Send + Sync {
    fn start(&self);
    fn force_break(&self);
    fn defer_break(&self, duration: Duration);
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
    /// Break type determined for the current work cycle once the prompt appears.
    pending_break_type: Option<BreakType>,
}

impl SchedulerInner {
    fn new(config: AppConfig) -> Self {
        SchedulerInner {
            state: SchedulerState::Idle,
            remaining_secs: config.work_interval_secs,
            work_cycles: 0,
            config,
            task: None,
            pending_break_type: None,
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
    prompt_remaining_secs: Arc<AtomicU64>,
}

impl TimerScheduler {
    pub fn new(bus: Arc<EventBus>, config: AppConfig) -> Arc<Self> {
        let scheduler = Arc::new(TimerScheduler {
            inner: Arc::new(Mutex::new(SchedulerInner::new(config))),
            bus: Arc::clone(&bus),
            prompt_remaining_secs: Arc::new(AtomicU64::new(0)),
        });

        // Subscribe to bus events (UserIdle, UserReturned, ConfigUpdated).
        scheduler.spawn_bus_listener(bus);

        scheduler
    }

    fn spawn_bus_listener(&self, bus: Arc<EventBus>) {
        let inner = Arc::clone(&self.inner);
        let bus_for_task = Arc::clone(&bus);
        let prompt_remaining_secs = Arc::clone(&self.prompt_remaining_secs);
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
                            g.pending_break_type = None;
                            tracing::info!("State transition: Working → Idle (user idle)");
                            Self::hide_prompt(&bus_for_task, &prompt_remaining_secs);
                            bus_for_task.emit(AppEvent::StateChanged(SchedulerState::Idle));
                        }
                    }
                    Ok(AppEvent::UserReturned) => {
                        let should_start = {
                            let g = inner.lock().unwrap();
                            g.state == SchedulerState::Idle
                        };
                        if should_start {
                            Self::do_start_working(&inner, &bus_for_task, &prompt_remaining_secs);
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
                            g.pending_break_type = None;
                            drop(g);
                            Self::spawn_work_task(
                                &inner,
                                &bus_for_task,
                                &prompt_remaining_secs,
                                remaining,
                            );
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

    fn hide_prompt(bus: &Arc<EventBus>, prompt_remaining_secs: &Arc<AtomicU64>) {
        if prompt_remaining_secs.swap(0, Ordering::Relaxed) != 0 {
            bus.emit(AppEvent::PreBreakPromptHidden);
        }
    }

    fn do_start_break(
        inner: &Arc<Mutex<SchedulerInner>>,
        bus: &Arc<EventBus>,
        prompt_remaining_secs: &Arc<AtomicU64>,
        break_type: Option<BreakType>,
    ) {
        let (break_type, break_secs) = {
            let mut g = inner.lock().unwrap();
            g.abort_task();
            let bt = break_type
                .or_else(|| g.pending_break_type.clone())
                .unwrap_or_else(|| g.next_break_type());
            g.work_cycles += 1;
            let secs = g.break_duration(&bt);
            g.state = SchedulerState::OnBreak;
            g.remaining_secs = secs;
            g.pending_break_type = None;
            (bt, secs)
        };
        Self::hide_prompt(bus, prompt_remaining_secs);
        tracing::info!("State transition: → OnBreak (forced, {break_type:?})");
        bus.emit(AppEvent::StateChanged(SchedulerState::OnBreak));
        bus.emit(AppEvent::BreakDue { break_type });
        Self::spawn_break_task(inner, bus, prompt_remaining_secs, break_secs);
    }

    fn spawn_break_task(
        inner: &Arc<Mutex<SchedulerInner>>,
        bus: &Arc<EventBus>,
        prompt_remaining_secs: &Arc<AtomicU64>,
        break_secs: u64,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_t = Arc::clone(&cancel);
        let inner_t = Arc::clone(inner);
        let bus_t = Arc::clone(bus);
        let prompt_remaining_t = Arc::clone(prompt_remaining_secs);

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
                g.pending_break_type = None;
                r
            };

            Self::hide_prompt(&bus_t, &prompt_remaining_t);
            tracing::info!("State transition: OnBreak → Working (break completed)");
            bus_t.emit(AppEvent::BreakCompleted);
            bus_t.emit(AppEvent::StateChanged(SchedulerState::Working));
            Self::spawn_work_task(&inner_t, &bus_t, &prompt_remaining_t, next_work_secs);
        });

        inner.lock().unwrap().task = Some(cancel);
    }

    fn do_start_working(
        inner: &Arc<Mutex<SchedulerInner>>,
        bus: &Arc<EventBus>,
        prompt_remaining_secs: &Arc<AtomicU64>,
    ) {
        let remaining = {
            let mut g = inner.lock().unwrap();
            g.abort_task();
            g.state = SchedulerState::Working;
            let r = g.config.work_interval_secs;
            g.remaining_secs = r;
            g.pending_break_type = None;
            r
        };
        Self::hide_prompt(bus, prompt_remaining_secs);
        tracing::info!("State transition: → Working");
        bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(inner, bus, prompt_remaining_secs, remaining);
    }

    fn spawn_work_task(
        inner: &Arc<Mutex<SchedulerInner>>,
        bus: &Arc<EventBus>,
        prompt_remaining_secs: &Arc<AtomicU64>,
        work_secs: u64,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_t = Arc::clone(&cancel);
        let inner_t = Arc::clone(inner);
        let bus_t = Arc::clone(bus);
        let prompt_remaining_t = Arc::clone(prompt_remaining_secs);

        crate::spawn_async(async move {
            // --- Work countdown ---
            for remaining in (0..work_secs).rev() {
                if cancel_t.load(Ordering::Relaxed) {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                let maybe_break_type = {
                    let mut g = inner_t.lock().unwrap();
                    g.remaining_secs = remaining;

                    if remaining <= PRE_BREAK_PROMPT_SECS {
                        let break_type = g.pending_break_type.clone().unwrap_or_else(|| {
                            let next = g.next_break_type();
                            g.pending_break_type = Some(next.clone());
                            next
                        });
                        Some(break_type)
                    } else {
                        g.pending_break_type = None;
                        None
                    }
                };

                if let Some(break_type) = maybe_break_type {
                    prompt_remaining_t.store(remaining, Ordering::Relaxed);
                    bus_t.emit(AppEvent::PreBreakPromptTick {
                        break_type,
                        remaining_secs: remaining,
                    });
                } else {
                    Self::hide_prompt(&bus_t, &prompt_remaining_t);
                }
            }
            if cancel_t.load(Ordering::Relaxed) {
                return;
            }

            // --- Transition to break ---
            let (break_type, break_secs) = {
                let mut g = inner_t.lock().unwrap();
                let bt = g.pending_break_type.clone().unwrap_or_else(|| g.next_break_type());
                g.work_cycles += 1;
                let secs = g.break_duration(&bt);
                g.state = SchedulerState::OnBreak;
                g.remaining_secs = secs;
                g.pending_break_type = None;
                (bt, secs)
            };

            Self::hide_prompt(&bus_t, &prompt_remaining_t);
            tracing::info!("State transition: Working → OnBreak ({break_type:?})");
            bus_t.emit(AppEvent::StateChanged(SchedulerState::OnBreak));
            bus_t.emit(AppEvent::BreakDue { break_type });
            Self::spawn_break_task(&inner_t, &bus_t, &prompt_remaining_t, break_secs);
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
        Self::do_start_working(&self.inner, &self.bus, &self.prompt_remaining_secs);
    }

    /// Force an immediate break regardless of current state.
    fn force_break(&self) {
        let current = self.inner.lock().unwrap().state.clone();
        if current == SchedulerState::OnBreak {
            tracing::debug!("force_break() called while already OnBreak — no-op");
            return;
        }
        tracing::info!("Force break requested");
        Self::do_start_break(&self.inner, &self.bus, &self.prompt_remaining_secs, None);
    }

    fn defer_break(&self, duration: Duration) {
        let secs = duration.as_secs();
        let current = self.inner.lock().unwrap().state.clone();
        if current != SchedulerState::Working {
            tracing::debug!("defer_break() called in state {current:?} — no-op");
            return;
        }

        tracing::info!("State transition: Working → Working (deferred {secs}s)");
        self.bus.emit(AppEvent::BreakDeferred { secs });

        {
            let mut g = self.inner.lock().unwrap();
            g.abort_task();
            g.state = SchedulerState::Working;
            g.remaining_secs = secs;
            g.pending_break_type = None;
        }

        Self::hide_prompt(&self.bus, &self.prompt_remaining_secs);
        self.bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(&self.inner, &self.bus, &self.prompt_remaining_secs, secs);
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
        Self::do_start_working(&self.inner, &self.bus, &self.prompt_remaining_secs);
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
            g.pending_break_type = None;
        }
        Self::hide_prompt(&self.bus, &self.prompt_remaining_secs);
        self.bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(&self.inner, &self.bus, &self.prompt_remaining_secs, secs);
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
        Self::hide_prompt(&self.bus, &self.prompt_remaining_secs);
        self.inner.lock().unwrap().pending_break_type = None;
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
        {
            let mut g = self.inner.lock().unwrap();
            g.state = SchedulerState::Working;
            g.pending_break_type = None;
        }
        tracing::info!("State transition: Paused → Working");
        self.bus.emit(AppEvent::StateChanged(SchedulerState::Working));
        Self::spawn_work_task(&self.inner, &self.bus, &self.prompt_remaining_secs, remaining);
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

        let mut saw_on_break = false;
        let mut saw_break_due = false;
        for _ in 0..8 {
            let ev = rx.recv().await.unwrap();
            match ev {
                AppEvent::StateChanged(SchedulerState::OnBreak) => saw_on_break = true,
                AppEvent::BreakDue {
                    break_type: BreakType::Short,
                } => {
                    saw_break_due = true;
                    break;
                }
                AppEvent::PreBreakPromptTick { .. } | AppEvent::PreBreakPromptHidden => {}
                other => panic!("unexpected event before break: {other:?}"),
            }
        }
        assert!(saw_on_break);
        assert!(saw_break_due);

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

        let mut reached_break = false;
        for _ in 0..8 {
            let ev = rx.recv().await.unwrap();
            if matches!(ev, AppEvent::BreakDue { .. }) {
                reached_break = true;
                break;
            }
        }
        assert!(reached_break);

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

        let mut reached_break = false;
        for _ in 0..8 {
            let ev = rx.recv().await.unwrap();
            if matches!(ev, AppEvent::BreakDue { .. }) {
                reached_break = true;
                break;
            }
        }
        assert!(reached_break);

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
        let mut saw_short_due = false;
        for _ in 0..6 {
            let ev = rx.recv().await.unwrap();
            if matches!(
                ev,
                AppEvent::BreakDue {
                    break_type: BreakType::Short,
                }
            ) {
                saw_short_due = true;
                break;
            }
        }
        assert!(saw_short_due);

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
        let mut saw_long_due = false;
        for _ in 0..6 {
            let ev = rx.recv().await.unwrap();
            if matches!(
                ev,
                AppEvent::BreakDue {
                    break_type: BreakType::Long,
                }
            ) {
                saw_long_due = true;
                break;
            }
        }
        assert!(saw_long_due);
    }

    #[tokio::test]
    async fn scheduler_emits_prompt_before_break_and_can_defer() {
        time::pause();
        let cfg = AppConfig {
            work_interval_secs: 35,
            break_duration_secs: 2,
            long_break_interval_secs: 70,
            long_break_duration_secs: 4,
            ..AppConfig::default()
        };
        let (sched, bus) = make(cfg);
        let mut rx = bus.subscribe();

        sched.start();
        rx.recv().await.unwrap(); // StateChanged(Working)

        time::advance(Duration::from_secs(4)).await;
        tokio::task::yield_now().await;

        let mut saw_prompt = false;
        for _ in 0..4 {
            let ev = rx.recv().await.unwrap();
            if matches!(
                ev,
                AppEvent::PreBreakPromptTick {
                    break_type: BreakType::Short,
                    remaining_secs: 31
                }
            ) {
                saw_prompt = true;
                break;
            }
        }
        assert!(saw_prompt);

        sched.defer_break(Duration::from_secs(60));

        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::BreakDeferred { secs: 60 }));
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::PreBreakPromptHidden));
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, AppEvent::StateChanged(SchedulerState::Working)));
        assert_eq!(sched.remaining_secs(), 60);
    }
}
