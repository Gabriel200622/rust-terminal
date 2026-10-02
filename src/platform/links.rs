//! Bounded, asynchronous handoff of terminal web links to the default browser.

use std::{
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

use eframe::egui;

pub const MAX_URL_BYTES: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebLink(String);

impl WebLink {
    pub fn new(url: &str) -> Option<Self> {
        if url.len() > MAX_URL_BYTES || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return None;
        }
        let prefix = if url
            .get(..8)
            .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
        {
            8
        } else if url
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
        {
            7
        } else {
            return None;
        };
        let authority = url[prefix..].split(['/', '?', '#']).next()?;
        if authority.is_empty() || authority.ends_with('@') || authority.starts_with(':') {
            return None;
        }
        Some(Self(url.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Default)]
pub struct LinkOpener {
    pending: Option<mpsc::Receiver<Result<(), &'static str>>>,
}

impl LinkOpener {
    /// Only one launcher can be in flight; URLs never enter diagnostics or saved state.
    pub fn open(&mut self, link: WebLink, ctx: egui::Context) -> Result<(), &'static str> {
        if self.pending.is_some() {
            return Err("A link is already opening. Try again in a moment.");
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("neptune-open-link".into())
            .spawn(move || {
                let result = launch(&link);
                let _ = sender.send(result);
                ctx.request_repaint();
            })
            .map_err(|_| "Could not start the browser launcher.")?;
        self.pending = Some(receiver);
        Ok(())
    }

    pub fn poll(&mut self) -> Option<Result<(), &'static str>> {
        let result = match self.pending.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("The browser launcher stopped unexpectedly.")
            }
        };
        self.pending = None;
        Some(result)
    }
}

fn browser_command(link: &WebLink) -> Command {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("/usr/bin/open");
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("rundll32.exe");
        command.arg("url.dll,FileProtocolHandler");
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        command
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut command = Command::new("xdg-open");
    command.arg(link.as_str());
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn launch(link: &WebLink) -> Result<(), &'static str> {
    let mut child = browser_command(link)
        .spawn()
        .map_err(|_| "Could not open the link. Check that a default browser is installed.")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => {
                return Err("Could not open the link. Check your default browser settings.");
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return if result.is_err() {
                    Err("Could not check the browser launcher.")
                } else {
                    Err("The browser launcher timed out. Check your default browser settings.")
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_links_accept_web_targets_and_reject_other_schemes_and_controls() {
        for url in [
            "https://example.com/a?q=one&x=two#section",
            "HTTP://localhost:8080",
            "https://[::1]/",
        ] {
            assert_eq!(WebLink::new(url).unwrap().as_str(), url);
        }
        for url in [
            "file:///tmp/file",
            "javascript:alert(1)",
            "https://",
            "https:///path",
            "https://example.com/\n",
            "https://example.com/a b",
        ] {
            assert!(WebLink::new(url).is_none(), "{url:?}");
        }
        assert!(
            WebLink::new(&format!(
                "https://example.com/{}",
                "x".repeat(MAX_URL_BYTES)
            ))
            .is_none()
        );
    }

    #[test]
    fn browser_handoff_passes_the_url_as_one_argument_without_a_shell() {
        let link = WebLink::new("https://example.com/?q=$(echo)&other=value").unwrap();
        let command = browser_command(&link);
        assert_eq!(command.get_args().last().unwrap(), link.as_str());
        #[cfg(target_os = "linux")]
        assert_eq!(command.get_program(), "xdg-open");
    }

    #[test]
    fn browser_requests_are_bounded_and_completion_allows_retry() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut opener = LinkOpener {
            pending: Some(receiver),
        };
        assert!(
            opener
                .open(
                    WebLink::new("https://example.com/").unwrap(),
                    egui::Context::default()
                )
                .is_err()
        );
        assert_eq!(opener.poll(), None);
        sender.send(Err("failed")).unwrap();
        assert_eq!(opener.poll(), Some(Err("failed")));
        assert!(opener.pending.is_none());
    }
}
