#![cfg(unix)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use terminal_core::{SessionOptions, SessionStatus, TerminalSession};

fn shell(script: &str) -> TerminalSession {
    TerminalSession::spawn(
        SessionOptions {
            shell: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
            cols: 80,
            rows: 12,
            scrollback: 128,
            ..SessionOptions::default()
        },
        Arc::new(|| {}),
    )
    .unwrap()
}

#[track_caller]
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "PTY condition timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn screen(session: &TerminalSession) -> String {
    session.screen_text()
}

#[test]
fn a_shell_reported_directory_survives_local_process_directory_polling() {
    let session = shell(
        "printf '\\033]7;file://remote/srv/remote%%20project\\007'; \
         while IFS= read -r line; do printf '\\033]7;file://remote/srv/next\\007'; done",
    );
    wait_for(|| session.metadata().reported_cwd.is_some());
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(
        session.metadata().reported_cwd.as_deref(),
        Some(std::path::Path::new("/srv/remote project"))
    );
    session.write(b"next\r").unwrap();
    wait_for(|| {
        session.metadata().reported_cwd.as_deref() == Some(std::path::Path::new("/srv/next"))
    });
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn real_pty_accepts_input_propagates_resize_and_reports_exit() {
    let session = shell(
        "printf '\\033]0;pty-smoke\\007\\033[38;2;91;166;201mREADY\\033[0m\\r\\n'; \
         IFS= read -r answer; printf 'GOT:%s\\r\\n' \"$answer\"; stty size; exit 7",
    );
    wait_for(|| screen(&session).contains("READY"));
    assert_eq!(session.metadata().title, "pty-smoke");
    session.resize(91, 17, 910, 340).unwrap();
    assert_eq!(session.viewport().columns, 91);
    assert_eq!(session.viewport().screen_lines, 17);
    session.write(b"hello\r").unwrap();
    wait_for(|| {
        matches!(
            session.metadata().status,
            SessionStatus::Exited { code: 7, .. }
        )
    });
    let output = screen(&session);
    assert!(output.contains("GOT:hello"), "{output}");
    assert!(
        output.contains("17 91"),
        "Child did not observe the resized PTY: {output}"
    );
    let metrics = session.metrics();
    assert!(metrics.bytes_received > 0);
    assert_eq!(metrics.bytes_received, metrics.bytes_parsed);
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn terminal_device_status_reply_reaches_the_real_child() {
    let session = shell(
        "stty raw -echo; printf '\\033[6n'; dd bs=1 count=6 2>/dev/null | od -An -tx1; printf '\\r\\n'",
    );
    wait_for(|| {
        matches!(
            session.metadata().status,
            SessionStatus::Exited { code: 0, .. }
        )
    });
    let output = screen(&session);
    // GNU and BSD od use different spacing; compare the actual reply bytes.
    let bytes: Vec<u8> = output
        .split_whitespace()
        .filter_map(|word| u8::from_str_radix(word, 16).ok())
        .collect();
    assert_eq!(bytes, b"\x1b[1;1R", "Missing ESC[1;1R reply: {output}");
}

#[test]
fn closing_an_idle_shell_terminates_every_io_worker() {
    let session = shell("printf 'READY\r\n'; sleep 30");
    wait_for(|| screen(&session).contains("READY"));
    wait_for(|| session.metrics().active_workers == 3);
    let started = Instant::now();
    session.shutdown();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "Shutdown blocked the UI"
    );
    wait_for(|| session.metrics().active_workers == 0);
    assert!(session.write(b"still alive?").is_err());
}

#[test]
fn closing_a_shell_ignoring_hup_escalates_and_reaps_it() {
    let session = shell("trap '' HUP; printf 'READY\r\n'; while :; do :; done");
    wait_for(|| screen(&session).contains("READY"));
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
    assert!(matches!(
        session.metadata().status,
        SessionStatus::Exited {
            signal: Some(_),
            ..
        }
    ));
}

#[test]
fn huge_input_is_rejected_before_it_can_block_the_ui() {
    let session = shell("printf 'READY\r\n'; sleep 30");
    wait_for(|| screen(&session).contains("READY"));
    assert!(session.write(&vec![b'x'; 1024 * 1024 + 1]).is_err());
    let started = Instant::now();
    session.write(&vec![b'x'; 512 * 1024]).unwrap();
    session.shutdown();
    assert!(started.elapsed() < Duration::from_millis(100));
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn unreasonable_grid_budgets_are_rejected_and_leave_a_live_session_unchanged() {
    let result = TerminalSession::spawn(
        SessionOptions {
            cols: 4096,
            rows: 4096,
            scrollback: 0,
            ..SessionOptions::default()
        },
        Arc::new(|| {}),
    );
    assert!(result.is_err());
    let session = shell("printf 'READY\r\n'; sleep 30");
    wait_for(|| screen(&session).contains("READY"));
    assert!(session.set_scrollback(1_000_000).is_err());
    assert!(session.resize(4096, 4096, 0, 0).is_err());
    assert_eq!(session.viewport().columns, 80);
    assert_eq!(session.viewport().screen_lines, 12);
    assert_eq!(session.metadata().status, SessionStatus::Running);
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn bracketed_paste_normalizes_lines_and_cannot_inject_an_end_delimiter() {
    let expected = b"\x1b[200~alpha\nbeta[201~\x1b[201~";
    let script = format!(
        "stty raw -echo; printf '\\033[?2004hREADY\\r\\n'; dd bs=1 count={} 2>/dev/null | od -An -tx1; printf '\\r\\n'",
        expected.len(),
    );
    let session = shell(&script);
    wait_for(|| screen(&session).contains("READY"));
    session.paste("alpha\r\nbeta\x1b[201~").unwrap();
    wait_for(|| {
        matches!(
            session.metadata().status,
            SessionStatus::Exited { code: 0, .. }
        )
    });
    let output = screen(&session);
    let bytes: Vec<u8> = output
        .split_whitespace()
        .filter_map(|word| u8::from_str_radix(word, 16).ok())
        .collect();
    assert_eq!(
        bytes, expected,
        "Unexpected paste bytes observed by the real child: {output}"
    );
}

#[test]
fn synchronized_output_timeout_wakes_without_additional_pty_bytes() {
    let callbacks = Arc::new(AtomicU64::new(0));
    let callback_count = callbacks.clone();
    let session = TerminalSession::spawn(
        SessionOptions {
            shell: Some("/bin/sh".into()),
            args: vec![
                "-c".into(),
                "stty raw -echo; printf READY; IFS= read -r start; \
                 printf '\\033[?2026hWAITING'; sleep 2"
                    .into(),
            ],
            ..SessionOptions::default()
        },
        Arc::new(move || {
            callback_count.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .unwrap();
    wait_for(|| session.metrics().bytes_parsed == 5 && callbacks.load(Ordering::Relaxed) == 1);
    session.acknowledge_repaint();
    let started = Instant::now();
    session.write(b"start\n").unwrap();
    wait_for(|| {
        // A PTY can split the begin-sync escape itself across reads and wake
        // for that prefix. Consume frames normally until the timeout makes the
        // buffered text visible; there are no later bytes to rescue a lost wake.
        session.acknowledge_repaint();
        screen(&session).contains("WAITING") && callbacks.load(Ordering::Relaxed) >= 2
    });
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "Buffered output was not flushed on its 150ms deadline"
    );
    assert!(screen(&session).contains("WAITING"));
    assert_eq!(
        session.metrics().bytes_parsed,
        5 + b"\x1b[?2026hWAITING".len() as u64
    );
    assert_eq!(session.metadata().status, SessionStatus::Running);
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn rejected_resize_preserves_grid_geometry_when_the_input_queue_is_full() {
    let session = shell("stty raw -echo; printf 'READY\\r\\n'; sleep 30");
    wait_for(|| screen(&session).contains("READY"));
    // The child does not read raw input. Fill the PTY's kernel input buffer,
    // block its writer, and then saturate the bounded command queue.
    session.write(&vec![b'x'; 512 * 1024]).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    let mut writes = 0;
    while session.write(&[b'x'; 1024]).is_ok() {
        writes += 1;
        assert!(writes < 10_000, "The input lane never applied backpressure");
    }
    assert!(session.resize(91, 17, 910, 340).is_err());
    assert_eq!(session.viewport().columns, 80);
    assert_eq!(session.viewport().screen_lines, 12);
    assert_eq!(session.metadata().status, SessionStatus::Running);
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn repaint_notifications_coalesce_bursts_and_force_the_final_frame() {
    let callbacks = Arc::new(AtomicU64::new(0));
    let callback_count = callbacks.clone();
    let session = TerminalSession::spawn(
        SessionOptions {
            shell: Some("/bin/sh".into()),
            args: vec![
                "-c".into(),
                "stty raw -echo; printf READY; \
                 IFS= read -r first; printf FIRST; \
                 IFS= read -r burst; i=0; while [ \"$i\" -lt 5000 ]; do \
                 printf 'x\\r\\n'; i=$((i + 1)); done; printf BURST-END; \
                 IFS= read -r finish; printf FINAL-FRAME; exit 7"
                    .into(),
            ],
            cols: 80,
            rows: 12,
            scrollback: 128,
            ..SessionOptions::default()
        },
        Arc::new(move || {
            callback_count.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .unwrap();
    wait_for(|| session.metrics().bytes_parsed == 5 && callbacks.load(Ordering::Relaxed) == 1);

    // No timer or subsequent child output is needed to notify the first glyph.
    // Acknowledge before snapshotting, matching the UI's frame contract.
    session.acknowledge_repaint();
    assert!(screen(&session).contains("READY"));
    session.write(b"first\n").unwrap();
    wait_for(|| session.metrics().bytes_parsed == 10 && callbacks.load(Ordering::Relaxed) == 2);
    assert_eq!(session.metadata().status, SessionStatus::Running);
    assert!(screen(&session).contains("FIRST"));
    let revision_before_burst = session.revision();

    // Leave the notification pending while a real PTY delivers thousands of
    // lines. Parsing, history, and revisions continue without more callbacks.
    session.write(b"burst\n").unwrap();
    wait_for(|| session.metrics().bytes_parsed == 10 + 5000 * 3 + 9);
    assert!(screen(&session).contains("BURST-END"));
    assert!(session.revision() > revision_before_burst);
    assert_eq!(session.history_size(), 128);
    assert_eq!(callbacks.load(Ordering::Relaxed), 2);

    // A pending notification must not hide natural exit or its final glyphs.
    session.write(b"finish\n").unwrap();
    wait_for(|| {
        matches!(
            session.metadata().status,
            SessionStatus::Exited { code: 7, .. }
        ) && callbacks.load(Ordering::Relaxed) == 3
    });
    assert!(screen(&session).contains("FINAL-FRAME"));
    assert_eq!(session.metrics().bytes_parsed, 10 + 5000 * 3 + 9 + 11);
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn acknowledging_before_a_snapshot_preserves_a_later_output_wakeup() {
    let callbacks = Arc::new(AtomicU64::new(0));
    let callback_count = callbacks.clone();
    let session = TerminalSession::spawn(
        SessionOptions {
            shell: Some("/bin/sh".into()),
            args: vec![
                "-c".into(),
                "stty raw -echo; printf READY; IFS= read -r next; printf LATER; sleep 30".into(),
            ],
            ..SessionOptions::default()
        },
        Arc::new(move || {
            callback_count.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .unwrap();
    wait_for(|| callbacks.load(Ordering::Relaxed) == 1);
    session.acknowledge_repaint();
    let old_frame = screen(&session);
    assert!(old_frame.contains("READY"));
    assert!(!old_frame.contains("LATER"));
    session.write(b"next\n").unwrap();
    wait_for(|| callbacks.load(Ordering::Relaxed) == 2);
    assert!(screen(&session).contains("LATER"));
    assert_eq!(session.metadata().status, SessionStatus::Running);
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn hidden_session_metadata_bypasses_a_pending_output_notification() {
    let callbacks = Arc::new(AtomicU64::new(0));
    let callback_count = callbacks.clone();
    let session = TerminalSession::spawn(
        SessionOptions {
            shell: Some("/bin/sh".into()),
            args: vec![
                "-c".into(),
                "stty raw -echo; printf READY; IFS= read -r next; \
                 printf '\\033]0;hidden-title\\007\\007'; sleep 30"
                    .into(),
            ],
            ..SessionOptions::default()
        },
        Arc::new(move || {
            callback_count.fetch_add(1, Ordering::Relaxed);
        }),
    )
    .unwrap();
    wait_for(|| callbacks.load(Ordering::Relaxed) == 1);
    // A hidden pane does not consume its pending output notification.
    session.write(b"next\n").unwrap();
    wait_for(|| callbacks.load(Ordering::Relaxed) == 3);
    let metadata = session.metadata();
    assert_eq!(metadata.title, "hidden-title");
    assert_eq!(metadata.bell_count, 1);
    assert_eq!(metadata.status, SessionStatus::Running);
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}

#[test]
fn zsh_multiline_prompt_resize_preserves_long_input_and_command_history() {
    // macOS ships zsh; Linux CI installs it for this real ZLE regression.
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(".zshrc"),
        r#"
autoload -Uz add-zsh-hook
pace_prompt_start() { printf '\033]133;A\007'; }
pace_command_start() { printf '\033]133;C\007'; }
add-zsh-hook precmd pace_prompt_start
add-zsh-hook preexec pace_command_start
setopt prompt_subst
PROMPT=$'PROMPT_LEFT${(pl.$((COLUMNS-21)).. .)}RIGHT_TAG\n> '
RPROMPT=
"#,
    )
    .unwrap();
    let session = TerminalSession::spawn(
        SessionOptions {
            shell: Some("zsh".into()),
            // Global compinit can prompt about insecure runner completion
            // directories. Load only this fixture's user startup files.
            args: vec!["-d".into(), "-i".into()],
            cwd: directory.path().into(),
            env: vec![(
                "ZDOTDIR".into(),
                directory.path().to_string_lossy().into_owned(),
            )],
            cols: 100,
            rows: 8,
            scrollback: 10_000,
        },
        Arc::new(|| {}),
    )
    .expect("zsh is required for the native multiline prompt regression");
    wait_for(|| screen(&session).contains("PROMPT_LEFT"));
    session
        .write(
            b"for i in {1..10050}; do printf 'KEEP_%05d\\n' $i; done; printf 'HISTORY_READY\\n'\r",
        )
        .unwrap();
    wait_for(|| {
        let text = screen(&session);
        text.contains("HISTORY_READY")
            && text.contains("KEEP_10050")
            && text.contains("PROMPT_LEFT")
            && session.history_size() == 10_000
    });

    let payload = format!("{}_END_TYPED", "abcdef0123456789".repeat(20));
    let command = format!("printf 'INPUT_OK_%s\\n' '{payload}'");
    session.write(command.as_bytes()).unwrap();
    wait_for(|| screen(&session).contains("END_TYPED"));
    for (cols, rows) in [
        (80, 8),
        (50, 8),
        (33, 8),
        (33, 4),
        (33, 8),
        (80, 8),
        (100, 8),
    ] {
        session.resize(cols, rows, 0, 0).unwrap();
        // Let real SIGWINCH/ZLE redraws finish before the next gesture. Rapid
        // queueing is covered separately from this prompt-reflow regression.
        std::thread::sleep(Duration::from_millis(100));
    }
    let text = screen(&session);
    assert_eq!(text.matches("PROMPT_LEFT").count(), 1, "{text}");
    assert_eq!(text.matches("RIGHT_TAG").count(), 1, "{text}");
    assert!(
        text.contains("END_TYPED"),
        "ZLE lost the input buffer: {text}"
    );
    session.write(b"\r").unwrap();
    wait_for(|| {
        let all = session.history_text();
        all.contains(&format!("INPUT_OK_{payload}")) && all.contains("KEEP_10050")
    });
    session.shutdown();
    wait_for(|| session.metrics().active_workers == 0);
}
