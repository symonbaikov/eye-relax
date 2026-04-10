use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::{Runtime, WebviewWindow};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as XprotoExt, GrabMode, GrabStatus, Window};
use x11rb::rust_connection::RustConnection;
use x11rb::CURRENT_TIME;

struct ActiveKeyboardGrab {
    conn: RustConnection,
    window: Window,
}

#[derive(Default)]
struct X11KeyboardGrabber {
    active: Mutex<Option<ActiveKeyboardGrab>>,
}

static KEYBOARD_GRABBER: OnceLock<X11KeyboardGrabber> = OnceLock::new();

pub fn try_grab_keyboard_for_overlay<R: Runtime>(window: &WebviewWindow<R>) -> Result<bool> {
    if !is_x11_session() {
        return Ok(false);
    }

    keyboard_grabber().grab(window)?;
    Ok(true)
}

pub fn release_keyboard_for_overlay() -> Result<()> {
    if !is_x11_session() {
        return Ok(());
    }

    keyboard_grabber().release()
}

fn keyboard_grabber() -> &'static X11KeyboardGrabber {
    KEYBOARD_GRABBER.get_or_init(X11KeyboardGrabber::default)
}

fn is_x11_session() -> bool {
    !matches!(
        std::env::var("XDG_SESSION_TYPE")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "wayland"
    )
}

impl X11KeyboardGrabber {
    fn grab<R: Runtime>(&self, window: &WebviewWindow<R>) -> Result<()> {
        let target_window = x11_window_id(window)?;
        let mut active = self.active.lock().unwrap();

        if active.as_ref().map(|grab| grab.window) == Some(target_window) {
            return Ok(());
        }

        if let Some(existing) = active.take() {
            ungrab_keyboard(&existing.conn)?;
        }

        let (conn, _) =
            RustConnection::connect(None).context("connect to the X11 server for keyboard grab")?;

        let reply = conn
            .grab_keyboard(
                false,
                target_window,
                CURRENT_TIME,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )
            .context("send XGrabKeyboard request")?
            .reply()
            .context("receive XGrabKeyboard reply")?;

        if reply.status != GrabStatus::SUCCESS {
            bail!("XGrabKeyboard rejected with status {:?}", reply.status);
        }

        conn.flush()
            .context("flush X11 keyboard grab request to the server")?;

        *active = Some(ActiveKeyboardGrab {
            conn,
            window: target_window,
        });

        Ok(())
    }

    fn release(&self) -> Result<()> {
        let mut active = self.active.lock().unwrap();

        if let Some(existing) = active.take() {
            ungrab_keyboard(&existing.conn)?;
        }

        Ok(())
    }
}

fn x11_window_id<R: Runtime>(window: &WebviewWindow<R>) -> Result<Window> {
    let handle = window
        .window_handle()
        .context("resolve raw window handle for overlay")?;

    match handle.as_raw() {
        RawWindowHandle::Xlib(handle) => {
            u32::try_from(handle.window).context("convert Xlib window id to X11 window")
        }
        RawWindowHandle::Xcb(handle) => Ok(handle.window.get()),
        other => bail!("overlay is not running on an X11 window backend: {other:?}"),
    }
}

fn ungrab_keyboard(conn: &RustConnection) -> Result<()> {
    conn.ungrab_keyboard(CURRENT_TIME)
        .context("send XUngrabKeyboard request")?;
    conn.flush()
        .context("flush X11 keyboard release request to the server")?;
    Ok(())
}
