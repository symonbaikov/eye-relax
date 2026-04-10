use std::sync::Arc;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use tokio::sync::broadcast;

use crate::events::{AppEvent, EventBus, SchedulerState};
use crate::scheduler::{SchedulerPort, TimerScheduler};

// ---------------------------------------------------------------------------
// Icon paths (embedded at compile time)
// ---------------------------------------------------------------------------

const ICON_WORKING: &[u8] = include_bytes!("../icons/tray/working.png");
const ICON_ONBREAK: &[u8] = include_bytes!("../icons/tray/onbreak.png");
const ICON_PAUSED: &[u8] = include_bytes!("../icons/tray/paused.png");

fn icon_for_state(state: &SchedulerState) -> Image<'static> {
    let bytes: &'static [u8] = match state {
        SchedulerState::Working => ICON_WORKING,
        SchedulerState::OnBreak => ICON_ONBREAK,
        SchedulerState::Idle | SchedulerState::Paused => ICON_PAUSED,
    };
    Image::from_bytes(bytes).expect("tray icon decode failed")
}

// ---------------------------------------------------------------------------
// Menu item IDs
// ---------------------------------------------------------------------------

const ID_PAUSE_RESUME: &str = "pause_resume";
const ID_BREAK_NOW: &str = "break_now";
const ID_SETTINGS: &str = "settings";
const ID_STATS: &str = "stats";
const ID_QUIT: &str = "quit";

// ---------------------------------------------------------------------------
// Build the initial tray
// ---------------------------------------------------------------------------

pub fn build_tray<R: Runtime>(
    app: &AppHandle<R>,
    scheduler: Arc<TimerScheduler>,
    bus: Arc<EventBus>,
) -> tauri::Result<TrayIcon<R>> {
    let state = scheduler.state();
    let icon = icon_for_state(&state);
    let tooltip = tooltip_for_state(&state, scheduler.remaining_secs());

    let menu = build_menu(app, &state)?;

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip(&tooltip)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event({
            let scheduler = Arc::clone(&scheduler);
            let app = app.clone();
            move |_tray, event| handle_menu_event(&app, &scheduler, event.id.as_ref())
        })
        .on_tray_icon_event(|_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Left click opens the menu (already handled by show_menu_on_left_click)
            }
        })
        .build(app)?;

    // Spawn task to keep tray icon/tooltip in sync with state changes.
    spawn_state_listener(tray.id().clone(), app.clone(), scheduler, bus);

    Ok(tray)
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    state: &SchedulerState,
) -> tauri::Result<Menu<R>> {
    let pause_label = if *state == SchedulerState::Paused {
        "Resume"
    } else {
        "Pause"
    };

    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "status", "LookAway", false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_PAUSE_RESUME, pause_label, true, None::<&str>)?,
            &MenuItem::with_id(app, ID_BREAK_NOW, "Break Now", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_SETTINGS, "Settings", true, None::<&str>)?,
            &MenuItem::with_id(app, ID_STATS, "Statistics", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_QUIT, "Quit", true, None::<&str>)?,
        ],
    )
}

fn tooltip_for_state(state: &SchedulerState, remaining: u64) -> String {
    match state {
        SchedulerState::Working => {
            let mins = remaining / 60;
            let secs = remaining % 60;
            format!("LookAway — Working ({mins:02}:{secs:02} remaining)")
        }
        SchedulerState::OnBreak => {
            format!("LookAway — Break ({remaining}s remaining)")
        }
        SchedulerState::Paused => "LookAway — Paused".to_string(),
        SchedulerState::Idle => "LookAway — Idle".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Menu event handler
// ---------------------------------------------------------------------------

fn handle_menu_event<R: Runtime>(
    app: &AppHandle<R>,
    scheduler: &Arc<TimerScheduler>,
    id: &str,
) {
    use crate::scheduler::SchedulerPort;

    match id {
        ID_PAUSE_RESUME => {
            let state = scheduler.state();
            if state == SchedulerState::Paused {
                tracing::info!("Tray: resume");
                scheduler.resume();
            } else {
                tracing::info!("Tray: pause");
                scheduler.pause();
            }
        }
        ID_BREAK_NOW => {
            tracing::info!("Tray: force break");
            scheduler.force_break();
        }
        ID_SETTINGS => {
            tracing::info!("Tray: open settings");
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        ID_STATS => {
            tracing::info!("Tray: open stats (via settings window)");
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        ID_QUIT => {
            tracing::info!("Tray: quit");
            app.exit(0);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Background listener — keeps icon and tooltip updated
// ---------------------------------------------------------------------------

fn spawn_state_listener<R: Runtime>(
    tray_id: tauri::tray::TrayIconId,
    app: AppHandle<R>,
    scheduler: Arc<TimerScheduler>,
    bus: Arc<EventBus>,
) {
    let mut rx = bus.subscribe();

    crate::spawn_async(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::StateChanged(new_state)) => {
                    let remaining = scheduler.remaining_secs();
                    let icon = icon_for_state(&new_state);
                    let tooltip = tooltip_for_state(&new_state, remaining);

                    if let Some(tray) = app.tray_by_id(&tray_id) {
                        let _ = tray.set_icon(Some(icon));
                        let _ = tray.set_tooltip(Some(&tooltip));

                        // Rebuild the menu to flip Pause↔Resume label.
                        if let Ok(menu) = build_menu(&app, &new_state) {
                            let _ = tray.set_menu(Some(menu));
                        }
                    }
                }
                Ok(AppEvent::BreakTick { remaining_secs }) => {
                    if let Some(tray) = app.tray_by_id(&tray_id) {
                        let tooltip =
                            tooltip_for_state(&SchedulerState::OnBreak, remaining_secs);
                        let _ = tray.set_tooltip(Some(&tooltip));
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Tray listener lagged {n} events");
                }
                _ => {}
            }
        }
    });
}
