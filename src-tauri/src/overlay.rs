use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, Runtime};
use tokio::sync::broadcast;

use crate::events::{AppEvent, BreakType, EventBus};

const OVERLAY_LABEL: &str = "overlay";

// ---------------------------------------------------------------------------
// Payloads sent to the frontend
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BreakDuePayload {
    break_type: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BreakTickPayload {
    remaining_secs: u64,
}

// ---------------------------------------------------------------------------
// Background listener
// ---------------------------------------------------------------------------

/// Spawns a task that bridges internal `AppEvent`s to Tauri window events.
///
/// Shows the overlay fullscreen window on break start, hides it on break end.
/// The React component uses polling to sync state after the window is shown,
/// avoiding any race conditions with event delivery.
pub fn spawn_overlay_listener<R: Runtime>(app: AppHandle<R>, bus: Arc<EventBus>) {
    let mut rx = bus.subscribe();

    crate::spawn_async(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::BreakDue { break_type }) => {
                    let break_type_str = match break_type {
                        BreakType::Short => "short",
                        BreakType::Long => "long",
                    };

                    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    // Also emit event for immediate update (best-effort).
                    // React polling will catch it if the event is missed.
                    let _ = app.emit_to(
                        EventTarget::webview(OVERLAY_LABEL),
                        "break-due",
                        BreakDuePayload {
                            break_type: break_type_str.to_string(),
                        },
                    );
                    tracing::info!("Overlay: shown for {} break", break_type_str);
                }
                Ok(AppEvent::BreakTick { remaining_secs }) => {
                    let _ = app.emit_to(
                        EventTarget::webview(OVERLAY_LABEL),
                        "break-tick",
                        BreakTickPayload { remaining_secs },
                    );
                }
                Ok(AppEvent::BreakCompleted)
                | Ok(AppEvent::BreakSkipped)
                | Ok(AppEvent::BreakSnoozed { .. }) => {
                    let _ = app.emit_to(EventTarget::webview(OVERLAY_LABEL), "break-completed", ());
                    // Wait for the 3-second CSS fade-out animation to finish.
                    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
                    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
                        let _ = window.hide();
                    }
                    tracing::info!("Overlay: hidden");
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Overlay listener lagged {n} events");
                }
                _ => {}
            }
        }
    });
}
