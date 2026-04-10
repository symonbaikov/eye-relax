pub mod wayland;
pub mod x11;

// ---------------------------------------------------------------------------
// ActivitySource trait
// ---------------------------------------------------------------------------

/// Platform abstraction for idle detection.
pub trait ActivitySource: Send + Sync {
    /// Seconds since the last user input event (keyboard or pointer).
    fn idle_seconds(&self) -> anyhow::Result<u64>;
    /// Whether the screen is currently locked.
    fn is_screen_locked(&self) -> anyhow::Result<bool>;
}

// ---------------------------------------------------------------------------
// Session type detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionType {
    Wayland,
    X11,
}

/// Detect the current display server by reading `$XDG_SESSION_TYPE`.
/// Falls back to X11 if the variable is absent or unrecognised.
pub fn detect_session_type() -> SessionType {
    match std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "wayland" => {
            tracing::info!("Detected session type: Wayland");
            SessionType::Wayland
        }
        _ => {
            tracing::info!("Detected session type: X11 (fallback)");
            SessionType::X11
        }
    }
}
