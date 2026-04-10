use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, LogicalPosition, Manager, Runtime};
use tokio::sync::broadcast;

use crate::events::{AppEvent, BreakType, EventBus};

const PROMPT_LABEL: &str = "prompt";
const PROMPT_WIDTH: f64 = 460.0;
const PROMPT_MARGIN_TOP: f64 = 40.0;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptTickPayload {
    break_type: String,
    remaining_secs: u64,
}

pub fn spawn_prompt_listener<R: Runtime>(app: AppHandle<R>, bus: Arc<EventBus>) {
    let mut rx = bus.subscribe();

    crate::spawn_async(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::PreBreakPromptTick {
                    break_type,
                    remaining_secs,
                }) => {
                    let break_type_str = match break_type {
                        BreakType::Short => "short",
                        BreakType::Long => "long",
                    };

                    if let Some(window) = app.get_webview_window(PROMPT_LABEL) {
                        if let Err(error) = position_prompt_window(&window) {
                            tracing::warn!("Failed to position prompt window: {error}");
                        }
                        let _ = window.show();
                    }

                    let _ = app.emit_to(
                        EventTarget::webview(PROMPT_LABEL),
                        "pre-break-prompt-tick",
                        PromptTickPayload {
                            break_type: break_type_str.to_string(),
                            remaining_secs,
                        },
                    );
                }
                Ok(AppEvent::PreBreakPromptHidden)
                | Ok(AppEvent::BreakDue { .. })
                | Ok(AppEvent::BreakCompleted)
                | Ok(AppEvent::BreakSkipped)
                | Ok(AppEvent::BreakDeferred { .. })
                | Ok(AppEvent::BreakSnoozed { .. }) => {
                    if let Some(window) = app.get_webview_window(PROMPT_LABEL) {
                        let _ = app.emit_to(
                            EventTarget::webview(PROMPT_LABEL),
                            "pre-break-prompt-hide",
                            (),
                        );
                        let _ = window.hide();
                    }
                }
                Ok(AppEvent::StateChanged(crate::events::SchedulerState::Idle))
                | Ok(AppEvent::StateChanged(crate::events::SchedulerState::Paused)) => {
                    if let Some(window) = app.get_webview_window(PROMPT_LABEL) {
                        let _ = app.emit_to(
                            EventTarget::webview(PROMPT_LABEL),
                            "pre-break-prompt-hide",
                            (),
                        );
                        let _ = window.hide();
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Prompt listener lagged {n} events");
                }
                _ => {}
            }
        }
    });
}

fn position_prompt_window<R: Runtime>(window: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
    let monitor = window
        .current_monitor()?
        .or_else(|| window.primary_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        let position = monitor.position();
        let size = monitor.size();
        let work_area = monitor.work_area();
        let x = position.x as f64 + (size.width as f64 - PROMPT_WIDTH) / 2.0;
        let y = work_area.position.y as f64 + PROMPT_MARGIN_TOP;
        window.set_position(LogicalPosition::new(x, y))?;
    }

    Ok(())
}
