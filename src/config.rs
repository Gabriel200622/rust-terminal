use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub theme: Theme,
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
    let persisted = temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("Cannot replace configuration {}", path.display()))?;
    drop(persisted);
    #[cfg(unix)]
    directory.sync_all().with_context(|| {
        format!(
            "Saved {}, but cannot synchronize its directory",
            path.display()
        )
    })?;
    Ok(())
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("dev", "Pace", "pace")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".pace"))
}

#[cfg(test)]
mod tests {
    use super::*;
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
    }
    #[test]
    fn saved_config_round_trip_replaces_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/config.toml");
        Config::default().save(&path).unwrap();
        let changed = Config {
            theme: Theme::Dusk,
            font_size: 19.0,
            ..Config::default()
        };
        changed.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.theme, Theme::Dusk);
        assert_eq!(loaded.font_size, 19.0);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
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
            atomic::{AtomicBool, Ordering},
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        const LENGTH: usize = 32 * 1024;
        atomic_write(&path, &vec![b'A'; LENGTH]).unwrap();
        let complete = AtomicBool::new(false);
        let start = Arc::new(Barrier::new(8));
        std::thread::scope(|scope| {
            let reader = scope.spawn(|| {
                while !complete.load(Ordering::Acquire) {
                    let bytes = std::fs::read(&path).unwrap();
                    assert_eq!(bytes.len(), LENGTH);
                    assert!(bytes.iter().all(|byte| *byte == bytes[0]));
                }
            });
            let writers: Vec<_> = (0..8)
                .map(|index| {
                    let path = &path;
                    let start = start.clone();
                    scope.spawn(move || {
                        start.wait();
                        for _ in 0..8 {
                            atomic_write(path, &vec![b'A' + index; LENGTH]).unwrap();
                        }
                    })
                })
                .collect();
            let results: Vec<_> = writers.into_iter().map(|writer| writer.join()).collect();
            complete.store(true, Ordering::Release);
            reader.join().unwrap();
            for result in results {
                result.unwrap();
            }
        });
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), LENGTH);
        assert!(bytes.iter().all(|byte| *byte == bytes[0]));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
