//! Restore helpers must never turn an interactive agent into a batch invocation.
#![cfg(unix)]

use neptune_model::{AgentKind, AgentSession};
use std::{os::unix::fs::PermissionsExt, sync::Arc, thread, time::Duration};
use terminal_core::{SessionOptions, TerminalSession};

#[test]
fn redirected_restore_leaves_a_quiet_shell_without_launching_the_agent() {
    for kind in [AgentKind::Claude, AgentKind::Codex] {
        // Keep the other descriptor on a real PTY, so either missing guard fails.
        for redirect in ["</dev/null", ">/dev/null"] {
            let root = tempfile::tempdir().unwrap();
            let launched = root.path().join("launched");
            let executable = root.path().join(kind.executable());
            std::fs::write(
                &executable,
                "#!/bin/sh\nprintf launched > \"$NEPTUNE_TEST_AGENT_LAUNCHED\"\nprintf 'UNEXPECTED_AGENT_OUTPUT\\n'\n",
            )
            .unwrap();
            std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
            let agent = AgentSession {
                kind,
                session_id: Some("11111111-1111-4111-8111-111111111111".into()),
                cwd: root.path().into(),
            };
            let options = SessionOptions {
                shell: Some("/bin/sh".into()),
                args: vec![
                    "-c".into(),
                    format!(
                        "\"$1\" --agent-restore \"$2\" {redirect}; status=$?; printf 'RESTORE_STATUS=%s\\n' \"$status\""
                    ),
                    "restore-test".into(),
                    env!("CARGO_BIN_EXE_neptune").into(),
                    serde_json::to_string(&agent).unwrap(),
                ],
                cwd: root.path().into(),
                env: vec![
                    ("PATH".into(), root.path().to_string_lossy().into_owned()),
                    (
                        "NEPTUNE_TEST_AGENT_LAUNCHED".into(),
                        launched.to_string_lossy().into_owned(),
                    ),
                    // Exercise the provider pass-through path if the guard regresses.
                    ("NEPTUNE_AGENT_RUN".into(), "restore-test".into()),
                ],
                ..Default::default()
            };
            let session = TerminalSession::spawn(options, Arc::new(|| {})).unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !session.screen_text().contains("RESTORE_STATUS=0") {
                assert!(
                    std::time::Instant::now() < deadline,
                    "restore did not return"
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(!launched.exists(), "{kind:?} launched with {redirect}");
            assert_eq!(session.screen_text().trim(), "RESTORE_STATUS=0");
            session.shutdown();
        }
    }
}
