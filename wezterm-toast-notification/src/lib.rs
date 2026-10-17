use std::sync::Arc;

mod dbus;
mod macos;
mod windows;

/// Called when the user clicks on a notification.
/// The parameter is an activation token supplied by the notification
/// server (an xdg-activation token on Wayland), if any, which can be
/// used to raise a window.
#[derive(Clone)]
pub struct ActivateHandler(Arc<dyn Fn(Option<String>) + Send + Sync>);

impl ActivateHandler {
    pub fn new<F: Fn(Option<String>) + Send + Sync + 'static>(f: F) -> Self {
        Self(Arc::new(f))
    }

    fn call(&self, token: Option<String>) {
        (self.0)(token)
    }
}

impl std::fmt::Debug for ActivateHandler {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str("ActivateHandler")
    }
}

#[derive(Debug, Clone)]
pub struct ToastNotification {
    pub title: String,
    pub message: String,
    pub url: Option<String>,
    pub timeout: Option<std::time::Duration>,
    /// When there is no timeout, whether the notification should
    /// remain on screen until dismissed rather than expiring after
    /// the system default duration
    pub persistent: bool,
    /// Called when the notification is clicked, if it has no url
    pub on_activate: Option<ActivateHandler>,
}

impl ToastNotification {
    pub fn show(self) {
        show(self)
    }
}

#[cfg(windows)]
use crate::windows as backend;
#[cfg(all(not(target_os = "macos"), not(windows)))]
use dbus as backend;
#[cfg(target_os = "macos")]
use macos as backend;

mod nop {
    use super::*;

    #[allow(dead_code)]
    pub fn show_notif(_: ToastNotification) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

pub fn show(notif: ToastNotification) {
    if let Err(err) = backend::show_notif(notif) {
        log::error!("Failed to show notification: {}", err);
    }
}

pub fn persistent_toast_notification_with_click_to_open_url(title: &str, message: &str, url: &str) {
    show(ToastNotification {
        title: title.to_string(),
        message: message.to_string(),
        url: Some(url.to_string()),
        timeout: None,
        persistent: true,
        on_activate: None,
    });
}

pub fn persistent_toast_notification(title: &str, message: &str) {
    show(ToastNotification {
        title: title.to_string(),
        message: message.to_string(),
        url: None,
        timeout: None,
        persistent: true,
        on_activate: None,
    });
}

#[cfg(target_os = "macos")]
pub use macos::initialize as macos_initialize;
