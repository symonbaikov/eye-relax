use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use crate::events::{AppEvent, EventBus};

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Abstraction over system notification delivery.
pub trait NotificationPort: Send + Sync {
    /// Show (or replace) the break notification.
    fn send_break(&self, title: &str, body: &str);
    /// Close the current notification (if any).
    fn close(&self);
}

// ---------------------------------------------------------------------------
// DbusNotifier — raw D-Bus via zbus (primary, with action hints)
// ---------------------------------------------------------------------------

pub struct DbusNotifier {
    conn: zbus::blocking::Connection,
    last_id: Mutex<u32>,
}

impl DbusNotifier {
    pub fn new() -> anyhow::Result<Self> {
        let conn = zbus::blocking::Connection::session()?;
        Ok(Self {
            conn,
            last_id: Mutex::new(0),
        })
    }
}

impl NotificationPort for DbusNotifier {
    fn send_break(&self, title: &str, body: &str) {
        let replaces_id: u32 = *self.last_id.lock().unwrap();
        let actions: Vec<&str> = vec!["skip", "Skip", "snooze", "Snooze 5m"];
        let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();

        match self.conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                "lookaway",
                replaces_id,
                "dialog-information",
                title,
                body,
                actions,
                hints,
                5000i32,
            ),
        ) {
            Ok(reply) => {
                if let Ok(new_id) = reply.body().deserialize::<u32>() {
                    *self.last_id.lock().unwrap() = new_id;
                }
            }
            Err(e) => {
                tracing::warn!("DbusNotifier: send failed: {e}");
            }
        }
    }

    fn close(&self) {
        let id = *self.last_id.lock().unwrap();
        if id == 0 {
            return;
        }
        let _ = self.conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "CloseNotification",
            &(id,),
        );
        *self.last_id.lock().unwrap() = 0;
    }
}

// ---------------------------------------------------------------------------
// LibnotifyNotifier — notify-rust (fallback, simpler)
// ---------------------------------------------------------------------------

pub struct LibnotifyNotifier {
    last_id: Mutex<u32>,
}

impl Default for LibnotifyNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl LibnotifyNotifier {
    pub fn new() -> Self {
        Self {
            last_id: Mutex::new(0),
        }
    }
}

impl NotificationPort for LibnotifyNotifier {
    fn send_break(&self, title: &str, body: &str) {
        let prev_id = *self.last_id.lock().unwrap();

        let mut notif = notify_rust::Notification::new();
        notif.summary(title).body(body).timeout(5000);

        // Replace existing notification if we have a previous ID.
        if prev_id > 0 {
            notif.id(prev_id);
        }

        match notif.show() {
            Ok(handle) => {
                *self.last_id.lock().unwrap() = handle.id();
            }
            Err(e) => {
                tracing::warn!("LibnotifyNotifier: send failed: {e}");
            }
        }
    }

    fn close(&self) {
        // notify-rust does not expose a close-by-id API on all platforms,
        // so we simply reset our tracking. The notification will expire.
        *self.last_id.lock().unwrap() = 0;
    }
}

// ---------------------------------------------------------------------------
// NotificationManager — EventBus → notifications bridge
// ---------------------------------------------------------------------------

/// Subscribes to the EventBus and fires system notifications for breaks.
///
/// Per the architecture, notifications complement the overlay and are sent
/// alongside it (fullscreen detection and conditional suppression are deferred
/// to a later phase).
pub fn spawn_notification_listener(notifier: Arc<dyn NotificationPort>, bus: Arc<EventBus>) {
    let mut rx = bus.subscribe();

    crate::spawn_async(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::BreakDue { .. }) => {
                    notifier.send_break(
                        "Time for a break",
                        "Look 20 feet away for 20 seconds. Rest your eyes.",
                    );
                }
                Ok(AppEvent::BreakCompleted)
                | Ok(AppEvent::BreakSkipped)
                | Ok(AppEvent::BreakSnoozed { .. }) => {
                    notifier.close();
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Notification listener lagged {n} events");
                }
                _ => {}
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Mock (tests)
// ---------------------------------------------------------------------------

/// Records calls for unit tests — no actual D-Bus interaction.
#[cfg(test)]
pub struct MockNotifier {
    pub sends: Mutex<Vec<(String, String, u32)>>, // (title, body, replaces_id)
    next_id: Mutex<u32>,
    pub last_id: Mutex<u32>,
}

#[cfg(test)]
impl Default for MockNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl MockNotifier {
    pub fn new() -> Self {
        Self {
            sends: Mutex::new(Vec::new()),
            next_id: Mutex::new(1),
            last_id: Mutex::new(0),
        }
    }

    /// Simulate the replaces_id that would be sent on the next call.
    pub fn current_replaces_id(&self) -> u32 {
        *self.last_id.lock().unwrap()
    }
}

#[cfg(test)]
impl NotificationPort for MockNotifier {
    fn send_break(&self, title: &str, body: &str) {
        let replaces_id = *self.last_id.lock().unwrap();
        self.sends
            .lock()
            .unwrap()
            .push((title.to_string(), body.to_string(), replaces_id));
        let new_id = {
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            current
        };
        *self.last_id.lock().unwrap() = new_id;
    }

    fn close(&self) {
        *self.last_id.lock().unwrap() = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_dedup_second_send_uses_previous_id() {
        let n = MockNotifier::new();

        n.send_break("Time for a break", "Look away.");
        let first_id = n.current_replaces_id();
        assert!(first_id > 0, "first send should assign an ID");

        n.send_break("Time for a break", "Look away.");
        let sends = n.sends.lock().unwrap();
        // Second call must have passed the first ID as replaces_id.
        assert_eq!(
            sends[1].2, first_id,
            "second send should replace the first notification"
        );
    }

    #[test]
    fn notification_close_resets_id() {
        let n = MockNotifier::new();
        n.send_break("break", "body");
        assert!(n.current_replaces_id() > 0);
        n.close();
        assert_eq!(n.current_replaces_id(), 0);
    }

    #[test]
    fn notification_after_close_creates_new() {
        let n = MockNotifier::new();
        n.send_break("a", "b");
        n.close();
        n.send_break("c", "d");
        let sends = n.sends.lock().unwrap();
        // After close, third send starts fresh (replaces_id == 0).
        assert_eq!(sends[1].2, 0, "after close, next send should not replace");
    }
}
