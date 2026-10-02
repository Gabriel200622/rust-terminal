//! Native OS notification delivery stays outside interactive frames.
use eframe::egui;
use neptune_model::PaneId;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

struct Request {
    title: String,
    body: String,
    live: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct DesktopNotifier {
    sender: Option<mpsc::SyncSender<Request>>,
    sessions: BTreeMap<PaneId, Arc<AtomicBool>>,
    results: Option<mpsc::Receiver<bool>>,
    last_sent: Option<Instant>,
    pub unavailable: bool,
}

impl DesktopNotifier {
    /// At most one OS banner per second globally; the in-app history is independent.
    pub fn show(&mut self, pane: PaneId, title: String, body: String, ctx: &egui::Context) {
        if self
            .last_sent
            .is_some_and(|last| last.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        if self.sender.is_none() {
            let (sender, receiver) = mpsc::sync_channel::<Request>(8);
            let (result_sender, results) = mpsc::sync_channel(8);
            let wake = ctx.clone();
            let worker = std::thread::Builder::new()
                .name("neptune-notifications".into())
                .spawn(move || {
                    let initialized = initialize();
                    while let Ok(request) = receiver.recv() {
                        if !request.live.load(Ordering::Acquire) {
                            continue;
                        }
                        let delivered = initialized && deliver(&request.title, &request.body);
                        let _ = result_sender.try_send(delivered);
                        wake.request_repaint();
                    }
                });
            if worker.is_err() {
                self.unavailable = true;
                return;
            }
            self.sender = Some(sender);
            self.results = Some(results);
        }
        if let Some(sender) = &self.sender {
            // On Linux the notification service interprets body markup. Escape
            // terminal-supplied text rather than allowing links or image markup.
            #[cfg(target_os = "linux")]
            let body = body
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let live = self
                .sessions
                .entry(pane)
                .or_insert_with(|| Arc::new(AtomicBool::new(true)))
                .clone();
            if sender.try_send(Request { title, body, live }).is_ok() {
                self.last_sent = Some(Instant::now());
            }
        }
    }

    /// Invalidate queued work before a pane is removed or its session replaced.
    pub fn cancel(&mut self, pane: PaneId) {
        if let Some(live) = self.sessions.remove(&pane) {
            live.store(false, Ordering::Release);
        }
    }

    pub fn cancel_all(&mut self) {
        for (_, live) in std::mem::take(&mut self.sessions) {
            live.store(false, Ordering::Release);
        }
    }

    pub fn poll(&mut self) {
        if let Some(results) = &self.results {
            for delivered in results.try_iter() {
                self.unavailable = !delivered;
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn initialize() -> bool {
    // Match the installed Neptune.app bundle, rather than impersonating Finder.
    mac_notification_sys::set_application("rs.neptune.terminal").is_ok()
}

#[cfg(windows)]
fn initialize() -> bool {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let result = (|| -> std::io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(r"Software\Classes\AppUserModelId\rs.neptune.terminal")?;
        key.set_value("DisplayName", &"Neptune")?;
        key.set_value("ShowInSettings", &1_u32)?;
        Ok(())
    })();
    result.is_ok()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn initialize() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn deliver(title: &str, body: &str) -> bool {
    // notify-rust's legacy macOS handle sends on Drop and hides send errors.
    // Call its native backend directly so the popover can report a failure.
    mac_notification_sys::Notification::new()
        .title(title)
        .message(body)
        .asynchronous(true)
        .send()
        .is_ok()
}

#[cfg(not(target_os = "macos"))]
fn deliver(title: &str, body: &str) -> bool {
    let mut notification = notify_rust::Notification::new();
    notification.appname("Neptune").summary(title).body(body);
    #[cfg(windows)]
    notification.app_id("rs.neptune.terminal");
    #[cfg(target_os = "linux")]
    notification.hint(notify_rust::Hint::DesktopEntry("neptune".into()));
    notification.show().is_ok()
}

impl Drop for DesktopNotifier {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notifications_cancel_queued_delivery_without_affecting_other_panes() {
        let mut notifier = DesktopNotifier::default();
        let old = Arc::new(AtomicBool::new(true));
        let other = Arc::new(AtomicBool::new(true));
        notifier.sessions.insert(PaneId::new(1), old.clone());
        notifier.sessions.insert(PaneId::new(2), other.clone());
        notifier.cancel(PaneId::new(1));
        assert!(!old.load(Ordering::Acquire));
        assert!(other.load(Ordering::Acquire));
        drop(notifier);
        assert!(!other.load(Ordering::Acquire));
    }
}
