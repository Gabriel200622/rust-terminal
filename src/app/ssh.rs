//! SSH launch commands. Remote paths are data, never terminal input or local cwd.
use std::path::Path;

use pace_model::Remote;

const BOOTSTRAP: &str = include_str!("ssh-bootstrap.sh");

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn arguments(remote: &Remote, cwd: Option<&Path>) -> Vec<String> {
    // Explicit commands need a remote PTY. Quote the entire script and the
    // path separately because OpenSSH sends a command string to the host shell.
    let directory = cwd.and_then(Path::to_str).unwrap_or_default();
    vec![
        "-t".into(),
        "--".into(),
        remote.destination().into(),
        format!("sh -c {} pace {}", quote(BOOTSTRAP), quote(directory)),
    ]
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use terminal_core::{SessionOptions, SessionStatus, TerminalSession};

    fn wait_for(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "SSH bootstrap condition timed out"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn bootstrap(home: &Path, dotdir: Option<&Path>, cwd: Option<&Path>) -> TerminalSession {
        let command = arguments(&Remote::parse("devbox").unwrap(), cwd).remove(3);
        let mut env = vec![
            ("SHELL".into(), "zsh".into()),
            ("HOME".into(), home.to_str().unwrap().into()),
            ("TMPDIR".into(), home.to_str().unwrap().into()),
            (
                "ZDOTDIR".into(),
                dotdir.unwrap_or(home).to_str().unwrap().into(),
            ),
        ];
        // Fixtures keep global zsh startup files but replace the user's files.
        env.push(("PACE_STARTUP".into(), String::new()));
        TerminalSession::spawn(
            SessionOptions {
                shell: Some("/bin/sh".into()),
                args: vec!["-c".into(), command],
                cwd: home.into(),
                env,
                ..Default::default()
            },
            Arc::new(|| {}),
        )
        .unwrap()
    }

    #[test]
    fn zsh_bootstrap_preserves_login_files_and_reports_directory_changes() {
        let root = tempfile::tempdir().unwrap();
        let dotdir = root.path().join("dotfiles");
        let project = root.path().join("project space ' % λ $(touch injected)");
        std::fs::create_dir(&dotdir).unwrap();
        std::fs::create_dir(&project).unwrap();
        for (file, stage) in [
            (".zshenv", "env"),
            (".zprofile", "profile"),
            (".zshrc", "rc"),
        ] {
            std::fs::write(
                dotdir.join(file),
                format!("PACE_STARTUP+=\"{stage} \"\nPROMPT='PACE> '\n"),
            )
            .unwrap();
        }
        std::fs::write(
            dotdir.join(".zlogin"),
            "printf '%slogin' \"$PACE_STARTUP\" > \"$HOME/startup\"\ncd \"$HOME\"\n",
        )
        .unwrap();
        let session = bootstrap(root.path(), Some(&dotdir), Some(&project));
        wait_for(|| session.metadata().reported_cwd.as_deref() == Some(project.as_path()));
        assert_eq!(
            std::fs::read_to_string(root.path().join("startup")).unwrap(),
            "env profile rc login"
        );
        assert!(!root.path().join("injected").exists());
        assert!(!std::fs::read_dir(root.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("pace-zsh.")
        }));
        session.write(b"cd -- \"$HOME\"\r").unwrap();
        wait_for(|| session.metadata().reported_cwd.as_deref() == Some(root.path()));
        session
            .write(b"printf '%s' \"$ZDOTDIR\" > \"$HOME/zdotdir\"; exit\r")
            .unwrap();
        wait_for(|| !matches!(session.metadata().status, SessionStatus::Running));
        assert_eq!(
            std::fs::read_to_string(root.path().join("zdotdir")).unwrap(),
            dotdir.to_str().unwrap()
        );
    }

    #[test]
    fn a_missing_remote_directory_fails_without_opening_a_shell_elsewhere() {
        let root = tempfile::tempdir().unwrap();
        let session = bootstrap(root.path(), None, Some(&root.path().join("missing")));
        wait_for(|| !matches!(session.metadata().status, SessionStatus::Running));
        assert!(
            matches!(session.metadata().status, SessionStatus::Exited { code, .. } if code != 0)
        );
        assert!(session.metadata().reported_cwd.is_none());
    }
}
