//! Explicit user-approved native installer/file-manager handoff.
use anyhow::Result;
#[cfg(not(windows))]
use anyhow::ensure;
#[cfg(not(windows))]
use std::time::{Duration, Instant};
use std::{
    path::Path,
    process::{Command, Stdio},
};

pub fn open_installer(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("/usr/bin/open");
        command.arg(path);
        command
    };
    #[cfg(windows)]
    let mut command = Command::new(path);
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().unwrap_or(path));
        command
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn()?;
    // The Windows installer is a separate interactive application. Do not wait
    // for installation or close terminal sessions on the user's behalf.
    #[cfg(windows)]
    {
        drop(child);
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let mut child = child;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait()? {
                ensure!(status.success(), "Installer handoff failed");
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("Installer handoff timed out");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}
