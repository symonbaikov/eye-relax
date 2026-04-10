use std::io::ErrorKind;
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Result};
use zbus::blocking::Connection;

pub fn suspend_system() -> Result<()> {
    let mut failures = Vec::new();

    if try_login1_dbus_suspend(&mut failures) {
        return Ok(());
    }

    if try_command("systemctl", &["suspend"], &mut failures) {
        return Ok(());
    }

    if try_command("loginctl", &["suspend"], &mut failures) {
        return Ok(());
    }

    if failures.is_empty() {
        bail!(
            "Could not suspend the system automatically. No supported suspend command was found."
        );
    }

    Err(anyhow!(
        "Could not suspend the system automatically. Tried: {}",
        failures.join("; ")
    ))
}

fn try_login1_dbus_suspend(failures: &mut Vec<String>) -> bool {
    let conn = match Connection::system() {
        Ok(conn) => conn,
        Err(error) => {
            failures.push(format!("system bus: {error}"));
            return false;
        }
    };

    match conn.call_method(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1",
        Some("org.freedesktop.login1.Manager"),
        "Suspend",
        &(false,),
    ) {
        Ok(_) => {
            tracing::info!("system suspend request accepted via login1 D-Bus");
            true
        }
        Err(error) => {
            tracing::warn!(error = %error, "system suspend failed via login1 D-Bus");
            failures.push(format!("login1 Suspend: {error}"));
            false
        }
    }
}

fn try_command(program: &str, args: &[&str], failures: &mut Vec<String>) -> bool {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    match command.status() {
        Ok(status) if status.success() => {
            tracing::info!(program, "system suspend request accepted via command");
            true
        }
        Ok(status) => {
            failures.push(format!("{program}: exited with status {status}"));
            false
        }
        Err(error) if error.kind() == ErrorKind::NotFound => false,
        Err(error) => {
            failures.push(format!("{program}: {error}"));
            false
        }
    }
}
