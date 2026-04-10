use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as ScreensaverExt;
use x11rb::rust_connection::RustConnection;

use super::ActivitySource;

/// X11 idle source using `XScreenSaverQueryInfo`.
/// Polls every 30 seconds via `ActivityTracker`.
pub struct X11IdleSource {
    conn: RustConnection,
    root: u32,
}

impl X11IdleSource {
    pub fn new() -> anyhow::Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)
            .map_err(|e| anyhow::anyhow!("X11 connect failed: {e}"))?;
        let root = conn.setup().roots[screen_num].root;
        Ok(X11IdleSource { conn, root })
    }
}

impl ActivitySource for X11IdleSource {
    fn idle_seconds(&self) -> anyhow::Result<u64> {
        let info = self
            .conn
            .screensaver_query_info(self.root)
            .map_err(|e| anyhow::anyhow!("XScreenSaverQueryInfo failed: {e}"))?
            .reply()
            .map_err(|e| anyhow::anyhow!("XScreenSaverQueryInfo reply failed: {e}"))?;

        // `ms_until_server` is millis since last input
        Ok(info.ms_since_user_input as u64 / 1000)
    }

    fn is_screen_locked(&self) -> anyhow::Result<bool> {
        let info = self
            .conn
            .screensaver_query_info(self.root)
            .map_err(|e| anyhow::anyhow!("XScreenSaverQueryInfo failed: {e}"))?
            .reply()
            .map_err(|e| anyhow::anyhow!("XScreenSaverQueryInfo reply failed: {e}"))?;

        // state: 0 = off, 1 = on (screen saver active), 2 = cycle, 3 = disabled
        Ok(info.state == 1)
    }
}
