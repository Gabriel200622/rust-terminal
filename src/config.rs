use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: Theme,
    pub accent: Accent,
    /// Scale of terminal content and window chrome, independent of font size.
    pub window_zoom: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub scrollback: usize,
    pub shell: Option<String>,
    pub cursor: Cursor,
    pub cursor_blink: bool,
    pub sidebar_width: f32,
    pub restore_workspaces: bool,
    pub confirm_close: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Graphite,
    Dusk,
    Light,
}

/// The highlight used for focus, selection and the terminal cursor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Accent {
    #[default]
    Blue,
    Indigo,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Graphite,
}

impl Accent {
    pub const ALL: [(Self, &'static str); 9] = [
        (Self::Blue, "Blue"),
        (Self::Indigo, "Indigo"),
        (Self::Purple, "Purple"),
        (Self::Pink, "Pink"),
        (Self::Red, "Red"),
        (Self::Orange, "Orange"),
        (Self::Yellow, "Yellow"),
        (Self::Green, "Green"),
        (Self::Graphite, "Graphite"),
    ];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cursor {
    #[default]
    Block,
    Beam,
    Underline,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::Graphite,
            accent: Accent::Blue,
            window_zoom: 1.0,
            font_size: 14.0,
            line_height: 1.4,
            scrollback: 10_000,
            shell: None,
            cursor: Cursor::Block,
            cursor_blink: false,
            sidebar_width: 216.0,
            restore_workspaces: true,
            confirm_close: true,
        }
    }
}

impl Config {
    // Match the bounds of the existing app zoom shortcuts.
    pub const WINDOW_ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.2..=5.0;

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut config: Self = toml::from_str(
            &std::fs::read_to_string(path)
                .with_context(|| format!("Cannot read {}", path.display()))?,
        )
        .with_context(|| format!("Invalid configuration in {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&mut self) -> Result<()> {
        anyhow::ensure!(
            self.window_zoom.is_finite() && Self::WINDOW_ZOOM_RANGE.contains(&self.window_zoom),
            "window_zoom must be between 0.2 and 5"
        );
        anyhow::ensure!(
            self.font_size.is_finite() && (9.0..=32.0).contains(&self.font_size),
            "font_size must be between 9 and 32"
        );
        anyhow::ensure!(
            self.line_height.is_finite() && (1.0..=2.0).contains(&self.line_height),
            "line_height must be between 1 and 2"
        );
        anyhow::ensure!(
            self.scrollback <= 1_000_000,
            "scrollback cannot exceed 1,000,000 lines"
        );
        anyhow::ensure!(
            self.sidebar_width.is_finite() && (170.0..=360.0).contains(&self.sidebar_width),
            "sidebar_width must be between 170 and 360"
        );
        if let Some(shell) = &self.shell {
            anyhow::ensure!(!shell.trim().is_empty(), "shell cannot be empty");
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        atomic_write(path, toml::to_string_pretty(self)?.as_bytes())
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    atomic_write_with(path, |file| file.write_all(bytes))
}

/// Prepare a complete file beside its destination and commit it with one rename.
/// Unique temporary files allow concurrent saves; the last successful commit wins.
/// Preparation/replacement failures preserve the previous destination and remove
/// the temporary file. A directory-sync failure can occur after the commit.
/// Windows access/sharing conflicts retry for up to 250 ms on the storage caller.
fn atomic_write_with(
    path: &Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Cannot create configuration directory {}", parent.display()))?;
    // Open before committing so a failure here also preserves the old file.
    #[cfg(unix)]
    let directory = std::fs::File::open(parent)
        .with_context(|| format!("Cannot open configuration directory {}", parent.display()))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".pace-save-")
        .tempfile_in(parent)
        .with_context(|| format!("Cannot prepare save for {}", path.display()))?;
    write(temporary.as_file_mut())
        .with_context(|| format!("Cannot write configuration {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("Cannot synchronize configuration {}", path.display()))?;
    // tempfile uses rename on Unix and MoveFileExW(REPLACE_EXISTING) on Windows.
    // Never remove the old destination before this atomic replacement.
    // Close our prepared file before replacement so another writer cannot
    // encounter our still-open, delete-pending destination on Windows.
    persist_temporary(temporary.into_temp_path(), path)
        .with_context(|| format!("Cannot replace configuration {}", path.display()))?;
    #[cfg(unix)]
    directory.sync_all().with_context(|| {
        format!(
            "Saved {}, but cannot synchronize its directory",
            path.display()
        )
    })?;
    Ok(())
}

fn persist_temporary(temporary: tempfile::TempPath, path: &Path) -> std::io::Result<()> {
    #[cfg(not(windows))]
    return temporary.persist(path).map_err(|error| error.error);

    #[cfg(windows)]
    {
        use std::time::{Duration, Instant};
        let deadline = Instant::now() + Duration::from_millis(250);
        let mut temporary = temporary;
        loop {
            match temporary.persist(path) {
                Ok(()) => return Ok(()),
                Err(error)
                    if matches!(error.error.raw_os_error(), Some(5 | 32 | 33))
                        && Instant::now() < deadline =>
                {
                    // MoveFileEx can temporarily reject replacement while a
                    // reader/another rename holds the destination. Keep the
                    // complete temporary file and never unlink the old data.
                    temporary = error.path;
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error.error),
            }
        }
    }
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "Pace", "pace")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".pace"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Atomic replacement guarantees complete contents, not that a racing
    // Windows open always succeeds. Test this Windows error policy on every
    // host, but apply it only to the Windows stress-test reader below.
    fn read_with_windows_retries(
        mut read: impl FnMut() -> std::io::Result<Vec<u8>>,
    ) -> std::io::Result<Vec<u8>> {
        use std::time::{Duration, Instant};

        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match read() {
                Err(error)
                    if matches!(error.raw_os_error(), Some(5 | 32 | 33))
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                result => return result,
            }
        }
    }

    #[test]
    fn concurrent_reader_recovers_from_windows_access_and_sharing_conflicts() {
        for code in [5, 32, 33] {
            let mut attempts = 0;
            let bytes = read_with_windows_retries(|| {
                attempts += 1;
                if attempts == 1 {
                    Err(std::io::Error::from_raw_os_error(code))
                } else {
                    Ok(b"complete file".to_vec())
                }
            })
            .unwrap();
            assert_eq!(bytes, b"complete file");
            assert_eq!(attempts, 2);
        }
    }

    #[test]
    fn concurrent_reader_surfaces_unexpected_errors_without_retrying() {
        for error in [
            std::io::Error::from_raw_os_error(2),
            std::io::Error::from_raw_os_error(3),
            std::io::Error::from_raw_os_error(87),
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "no Windows error code",
            ),
            std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid file"),
        ] {
            let mut error = Some(error);
            let raw_code = error.as_ref().unwrap().raw_os_error();
            let kind = error.as_ref().unwrap().kind();
            let returned = read_with_windows_retries(|| {
                Err(error.take().expect("Unexpected error was retried"))
            })
            .unwrap_err();
            assert_eq!(returned.raw_os_error(), raw_code);
            assert_eq!(returned.kind(), kind);
        }
    }

    #[test]
    fn concurrent_reader_surfaces_persistent_access_denial_after_timeout() {
        let started = std::time::Instant::now();
        let mut attempts = 0;
        let error = read_with_windows_retries(|| {
            attempts += 1;
            Err(std::io::Error::from_raw_os_error(5))
        })
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(5));
        assert!(attempts > 1);
        // Each retry sleeps at least 5 ms within the 250 ms budget.
        assert!(attempts <= 51);
        assert!(started.elapsed() >= std::time::Duration::from_millis(250));
    }

    #[cfg(windows)]
    #[test]
    fn concurrent_reader_recovers_after_a_windows_handle_releases_read_access() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        std::fs::write(&path, b"complete file").unwrap();
        let mut blocker = Some(
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&path)
                .unwrap(),
        );
        let bytes = read_with_windows_retries(|| {
            let result = std::fs::read(&path);
            if let Err(error) = &result {
                assert_eq!(error.raw_os_error(), Some(32));
                // Release only after a real sharing conflict, without making
                // recovery depend on another thread meeting the retry deadline.
                drop(blocker.take().expect("Read stayed blocked after release"));
            }
            result
        })
        .unwrap();
        assert!(blocker.is_none());
        assert_eq!(bytes, b"complete file");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn save_retries_a_temporary_windows_sharing_violation() {
        use std::io::Write;
        use std::os::windows::fs::OpenOptionsExt;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        std::fs::write(&path, b"previous complete file").unwrap();
        // FILE_SHARE_READ | FILE_SHARE_WRITE, deliberately without DELETE.
        let reader = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();
        let (prepared, ready) = std::sync::mpsc::sync_channel(1);
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                atomic_write_with(&path, |file| {
                    file.write_all(b"replacement complete file")?;
                    prepared.send(()).unwrap();
                    Ok(())
                })
            });
            ready.recv_timeout(Duration::from_secs(5)).unwrap();
            std::thread::sleep(Duration::from_millis(50));
            assert_eq!(std::fs::read(&path).unwrap(), b"previous complete file");
            drop(reader);
            writer.join().unwrap().unwrap();
        });
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement complete file");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn sharing_timeout_preserves_previous_file_and_cleans_tempfile() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        std::fs::write(&path, b"previous complete file").unwrap();
        // Keep the destination open without FILE_SHARE_DELETE past the retry limit.
        let reader = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1 | 2)
            .open(&path)
            .unwrap();

        let error = atomic_write(&path, b"replacement complete file").unwrap_err();
        assert!(matches!(
            error
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(5 | 32 | 33)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"previous complete file");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

        drop(reader);
        atomic_write(&path, b"replacement complete file").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement complete file");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn rejects_invalid_resources() {
        let mut c = Config {
            font_size: f32::NAN,
            ..Config::default()
        };
        assert!(c.validate().is_err());
        c = Config {
            scrollback: usize::MAX,
            ..Config::default()
        };
        assert!(c.validate().is_err());
        assert!(toml::from_str::<Config>("font_szie = 14").is_err());
    }
    #[test]
    fn config_round_trip() {
        let c = Config::default();
        let parsed: Config = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
        assert_eq!(parsed.font_size, 14.0);
        assert_eq!(parsed.theme, Theme::Graphite);
        assert_eq!(parsed.accent, Accent::Blue);
    }
    #[test]
    fn configuration_saved_before_accents_still_loads() {
        let parsed: Config = toml::from_str("theme = \"dusk\"\nfont_size = 15.0\n").unwrap();
        assert_eq!(parsed.theme, Theme::Dusk);
        assert_eq!(parsed.accent, Accent::Blue);
        assert_eq!(parsed.window_zoom, 1.0);
        let accent: Config = toml::from_str("accent = \"indigo\"").unwrap();
        assert_eq!(accent.accent, Accent::Indigo);
    }
    #[test]
    fn saved_config_round_trip_replaces_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/config.toml");
        Config::default().save(&path).unwrap();
        let changed = Config {
            theme: Theme::Dusk,
            window_zoom: 1.3,
            font_size: 19.0,
            ..Config::default()
        };
        changed.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.theme, Theme::Dusk);
        assert_eq!(loaded.window_zoom, 1.3);
        assert_eq!(loaded.font_size, 19.0);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }
    #[test]
    fn window_zoom_rejects_non_finite_and_out_of_range_values() {
        for zoom in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 0.19, 5.01] {
            let mut config = Config {
                window_zoom: zoom,
                ..Config::default()
            };
            assert!(config.validate().is_err(), "Accepted zoom {zoom}");
        }
        for zoom in [0.2, 1.0, 1.35, 5.0] {
            let mut config = Config {
                window_zoom: zoom,
                ..Config::default()
            };
            assert!(config.validate().is_ok(), "Rejected zoom {zoom}");
        }
    }
    #[test]
    fn failed_preparation_preserves_previous_file() {
        use std::io::Write;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        atomic_write(&path, b"previous complete configuration").unwrap();
        let result = atomic_write_with(&path, |file| {
            file.write_all(b"incomplete replacement")?;
            Err(std::io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"previous complete configuration"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[test]
    fn failed_replacement_preserves_destination_and_cleans_tempfile() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("existing-directory");
        std::fs::create_dir(&destination).unwrap();
        let previous = destination.join("config.toml");
        std::fs::write(&previous, b"previous file").unwrap();
        assert!(atomic_write(&destination, b"replacement").is_err());
        assert_eq!(std::fs::read(&previous).unwrap(), b"previous file");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[test]
    fn concurrent_saves_never_expose_partial_files_or_collide() {
        use std::sync::{
            Arc, Barrier,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        const LENGTH: usize = 32 * 1024;
        atomic_write(&path, &vec![b'A'; LENGTH]).unwrap();
        let complete = AtomicBool::new(false);
        let committed = AtomicUsize::new(0);
        let start = Arc::new(Barrier::new(9));
        std::thread::scope(|scope| {
            let reader = scope.spawn(|| {
                start.wait();
                let mut observed = 0;
                loop {
                    #[cfg(windows)]
                    let bytes = read_with_windows_retries(|| std::fs::read(&path)).unwrap();
                    #[cfg(not(windows))]
                    let bytes = std::fs::read(&path).unwrap();
                    assert_eq!(bytes.len(), LENGTH);
                    assert!((b'A'..=b'H').contains(&bytes[0]));
                    assert!(bytes.iter().all(|byte| *byte == bytes[0]));
                    observed += 1;
                    if complete.load(Ordering::Acquire) {
                        return observed;
                    }
                }
            });
            let writers: Vec<_> = (0..8)
                .map(|index| {
                    let path = &path;
                    let start = start.clone();
                    let committed = &committed;
                    scope.spawn(move || {
                        start.wait();
                        for _ in 0..8 {
                            match atomic_write(path, &vec![b'A' + index; LENGTH]) {
                                Ok(()) => {
                                    committed.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(error) => {
                                    // Continuous readers/writers can exhaust the bounded
                                    // Windows sharing retries. Every other failure is a bug.
                                    #[cfg(windows)]
                                    let sharing_conflict = error
                                        .to_string()
                                        .starts_with("Cannot replace configuration ")
                                        && error.downcast_ref::<std::io::Error>().is_some_and(
                                            |error| {
                                                matches!(error.raw_os_error(), Some(5 | 32 | 33))
                                            },
                                        );
                                    #[cfg(not(windows))]
                                    let sharing_conflict = false;
                                    assert!(sharing_conflict, "Concurrent save failed: {error:#}");
                                }
                            }
                        }
                    })
                })
                .collect();
            let results: Vec<_> = writers.into_iter().map(|writer| writer.join()).collect();
            complete.store(true, Ordering::Release);
            assert!(reader.join().unwrap() > 0);
            for result in results {
                result.unwrap();
            }
        });
        assert!(committed.load(Ordering::Relaxed) > 0);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), LENGTH);
        assert!(bytes.iter().all(|byte| *byte == bytes[0]));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
