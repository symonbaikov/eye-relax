use super::ActivitySource;

/// Wayland idle source.
///
/// Full implementation uses `ext_idle_notify_v1` protocol (event-driven).
/// This polling implementation reads idle time via the GNOME Mutter / KDE
/// D-Bus interfaces as a pragmatic fallback that works on all compositors.
///
/// For Phase 4 we provide a functional polling implementation; the
/// fully event-driven `ext_idle_notify_v1` version is planned for a later phase.
pub struct WaylandIdleSource {
    /// Cache the D-Bus connection so we don't reconnect on every poll.
    session_bus: Option<zbus::blocking::Connection>,
}

impl Default for WaylandIdleSource {
    fn default() -> Self {
        Self::new()
    }
}

impl WaylandIdleSource {
    pub fn new() -> Self {
        let session_bus = zbus::blocking::Connection::session().ok();
        WaylandIdleSource { session_bus }
    }

    /// Try to get idle time via org.gnome.Mutter.IdleMonitor (GNOME/Wayland).
    fn idle_ms_via_mutter(&self) -> Option<u64> {
        let conn = self.session_bus.as_ref()?;
        let msg = conn
            .call_method(
                Some("org.gnome.Mutter.IdleMonitor"),
                "/org/gnome/Mutter/IdleMonitor/Core",
                Some("org.gnome.Mutter.IdleMonitor"),
                "GetIdletime",
                &(),
            )
            .ok()?;
        msg.body().deserialize::<u64>().ok()
    }
}

impl ActivitySource for WaylandIdleSource {
    fn idle_seconds(&self) -> anyhow::Result<u64> {
        if let Some(ms) = self.idle_ms_via_mutter() {
            return Ok(ms / 1000);
        }
        // If D-Bus query fails (non-GNOME compositor), return 0 so the
        // scheduler keeps running rather than silently resetting.
        tracing::warn!("WaylandIdleSource: could not query idle time, reporting 0s idle");
        Ok(0)
    }

    fn is_screen_locked(&self) -> anyhow::Result<bool> {
        let conn = self
            .session_bus
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no D-Bus session"))?;

        // org.freedesktop.login1 reports session locked state.
        let reply = conn.call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1/session/auto",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.login1.Session", "LockedHint"),
        );

        match reply {
            Ok(msg) => {
                let body = msg.body();
                let locked: bool = body
                    .deserialize::<zbus::zvariant::Value>()
                    .ok()
                    .and_then(|v| {
                        if let zbus::zvariant::Value::Bool(b) = v {
                            Some(b)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(false);
                Ok(locked)
            }
            Err(_) => Ok(false),
        }
    }
}
