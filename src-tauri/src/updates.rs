use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use semver::Version;
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

use crate::notifications::NotificationPort;

const RELEASES_URL: &str = "https://github.com/symonbaikov/eye-relax/releases";
const UPDATE_ENDPOINT: &str =
    "https://github.com/symonbaikov/eye-relax/releases/latest/download/latest.json";
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
const UPDATE_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallType {
    AppImage,
    SystemPkg,
}

impl InstallType {
    fn as_str(self) -> &'static str {
        match self {
            Self::AppImage => "appimage",
            Self::SystemPkg => "system_pkg",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub pub_date: String,
    pub install_type: String,
}

#[derive(Debug, serde::Deserialize)]
struct LatestRelease {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    pub_date: Option<String>,
}

pub fn detect_install_type() -> InstallType {
    if std::env::var_os("APPIMAGE").is_some() {
        InstallType::AppImage
    } else {
        InstallType::SystemPkg
    }
}

pub async fn check_for_update<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<UpdateInfo>, String> {
    match detect_install_type() {
        InstallType::AppImage => fetch_appimage_update(app).await,
        InstallType::SystemPkg => fetch_system_package_update(app).await,
    }
}

pub async fn install_update<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    match detect_install_type() {
        InstallType::AppImage => {
            let updater = app.updater().map_err(|error| error.to_string())?;
            let Some(update) = updater.check().await.map_err(|error| error.to_string())? else {
                return Err("No update available.".to_string());
            };

            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|error| error.to_string())?;
            app.restart()
        }
        InstallType::SystemPkg => app
            .opener()
            .open_url(RELEASES_URL, None::<&str>)
            .map_err(|error| error.to_string()),
    }
}

pub fn spawn_update_checker<R: Runtime>(app: AppHandle<R>, notifier: Arc<dyn NotificationPort>) {
    crate::spawn_async(async move {
        let mut last_notified_version: Option<String> = None;

        loop {
            match check_for_update(&app).await {
                Ok(Some(update)) => {
                    if let Err(error) = app.emit("update-available", update.clone()) {
                        tracing::warn!("Failed to emit update event: {error}");
                    }

                    let notification_key = format!("{}:{}", update.install_type, update.version);
                    if last_notified_version.as_deref() != Some(notification_key.as_str()) {
                        notifier.send_update(
                            &format!("Blinkly {} is available", update.version),
                            &notification_body(&update),
                        );
                        last_notified_version = Some(notification_key);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!("Update check failed: {error}");
                }
            }

            tokio::time::sleep(UPDATE_CHECK_INTERVAL).await;
        }
    });
}

async fn fetch_appimage_update<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<UpdateInfo>, String> {
    let updater = app.updater().map_err(|error| error.to_string())?;
    let update = updater.check().await.map_err(|error| error.to_string())?;

    Ok(update.map(|update| UpdateInfo {
        version: trim_version_prefix(&update.version).to_string(),
        notes: update.body.unwrap_or_default(),
        pub_date: update.date.map(|date| date.to_string()).unwrap_or_default(),
        install_type: InstallType::AppImage.as_str().to_string(),
    }))
}

async fn fetch_system_package_update<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<UpdateInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(UPDATE_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(UPDATE_ENDPOINT)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    match response.status() {
        StatusCode::NO_CONTENT => return Ok(None),
        status if !status.is_success() => {
            return Err(format!("Update endpoint returned {status}"));
        }
        _ => {}
    }

    let release = response
        .json::<LatestRelease>()
        .await
        .map_err(|error| error.to_string())?;

    let current_version = app.package_info().version.to_string();
    if !is_newer_version(&current_version, &release.version)? {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        version: trim_version_prefix(&release.version).to_string(),
        notes: release.notes.unwrap_or_default(),
        pub_date: release.pub_date.unwrap_or_default(),
        install_type: InstallType::SystemPkg.as_str().to_string(),
    }))
}

fn notification_body(update: &UpdateInfo) -> String {
    if update.install_type == InstallType::AppImage.as_str() {
        "Download and install the latest AppImage from Blinkly settings.".to_string()
    } else {
        "Open Blinkly settings to download the latest .deb or .rpm release.".to_string()
    }
}

fn is_newer_version(current_version: &str, candidate_version: &str) -> Result<bool, String> {
    let current = parse_version(current_version)?;
    let candidate = parse_version(candidate_version)?;
    Ok(candidate > current)
}

fn parse_version(version: &str) -> Result<Version, String> {
    Version::parse(trim_version_prefix(version)).map_err(|error| error.to_string())
}

fn trim_version_prefix(version: &str) -> &str {
    version.trim().trim_start_matches('v')
}

#[cfg(test)]
mod tests {
    use super::{is_newer_version, trim_version_prefix};

    #[test]
    fn version_prefix_is_trimmed() {
        assert_eq!(trim_version_prefix("v0.2.0"), "0.2.0");
        assert_eq!(trim_version_prefix("0.2.0"), "0.2.0");
    }

    #[test]
    fn newer_versions_are_detected() {
        assert!(is_newer_version("0.1.0", "0.2.0").unwrap());
        assert!(!is_newer_version("0.2.0", "0.2.0").unwrap());
        assert!(!is_newer_version("0.3.0", "0.2.0").unwrap());
    }
}
