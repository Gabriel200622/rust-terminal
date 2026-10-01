//! Window geometry is independent of workspace restoration and preferences.

use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowState {
    pub version: u32,
    /// Logical dimensions of the last non-maximized window.
    pub inner_size: [f32; 2],
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            version: 1,
            inner_size: [1180.0, 760.0],
            maximized: false,
        }
    }
}

impl WindowState {
    pub fn valid_size(size: [f32; 2]) -> bool {
        size[0].is_finite()
            && size[1].is_finite()
            && (640.0..=8192.0).contains(&size[0])
            && (400.0..=8192.0).contains(&size[1])
    }

    /// Minimized/fullscreen dimensions must never replace the normal size.
    pub fn observe(
        &mut self,
        size: [f32; 2],
        maximized: Option<bool>,
        minimized: bool,
        fullscreen: bool,
    ) -> bool {
        if minimized || fullscreen {
            return false;
        }
        let Some(maximized) = maximized else {
            return false;
        };
        let previous = *self;
        self.maximized = maximized;
        if !maximized && Self::valid_size(size) {
            self.inner_size = size;
        }
        *self != previous
    }
}

#[derive(Default)]
pub struct LoadReport {
    pub state: WindowState,
    pub error: Option<String>,
    pub can_write: bool,
}

pub fn load(path: &Path) -> LoadReport {
    match read(path) {
        Ok(state) => LoadReport {
            state,
            can_write: true,
            error: None,
        },
        Err(error) => LoadReport {
            error: Some(format!(
                "Cannot restore window from {}; saved file will be preserved: {error}",
                path.display()
            )),
            ..LoadReport::default()
        },
    }
}

fn read(path: &Path) -> anyhow::Result<WindowState> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WindowState::default());
        }
        Err(error) => return Err(error.into()),
    };
    // Geometry needs only a few bytes. Bound reads of damaged/untrusted files.
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 4096, "window state exceeds 4096 bytes");
    let state: WindowState = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(state.version == 1, "unsupported window state version");
    anyhow::ensure!(
        WindowState::valid_size(state.inner_size),
        "invalid window size"
    );
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximize_and_minimize_preserve_the_resized_normal_dimensions() {
        let mut state = WindowState::default();
        assert!(state.observe([900.0, 640.0], Some(false), false, false));
        assert!(state.observe([1920.0, 1080.0], Some(true), false, false));
        assert!(!state.observe([0.0, 0.0], Some(false), true, false));
        assert!(!state.observe([1920.0, 1080.0], Some(false), false, true));
        assert!(!state.observe([1180.0, 760.0], None, false, false));
        assert_eq!(state.inner_size, [900.0, 640.0]);
        assert!(state.maximized);
        assert!(state.observe([900.0, 640.0], Some(false), false, false));
        assert!(!state.observe([900.0, 640.0], Some(false), false, false));
    }

    #[test]
    fn missing_state_uses_the_default_window_and_allows_saving() {
        let directory = tempfile::tempdir().unwrap();
        let report = load(&directory.path().join("window.json"));
        assert_eq!(report.state, WindowState::default());
        assert!(report.can_write);
        assert!(report.error.is_none());
    }

    #[test]
    fn damaged_unsupported_and_unreadable_state_remain_protected() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("window.json");
        for bytes in [
            b"broken".to_vec(),
            br#"{"version":2,"inner_size":[900,640],"maximized":true}"#.to_vec(),
            br#"{"version":1,"inner_size":[0,640],"maximized":true}"#.to_vec(),
            vec![b' '; 4097],
        ] {
            std::fs::write(&path, &bytes).unwrap();
            let report = load(&path);
            assert!(!report.can_write);
            assert!(report.error.is_some());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        assert!(!load(directory.path()).can_write);
    }
}
