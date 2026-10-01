//! `cargo run --release -p terminal-core --example pty_bench`
//!
//! Unix pipeline benchmark: actual child output -> native PTY -> bounded reader
//! -> VT parser + 10,000 history rows. Requires /bin/sh, stty, and POSIX awk.
//! Timing begins after a raw-mode/ready handshake and before releasing the child
//! producer. It includes start-input dispatch and awk launch/production, and ends
//! only after the child is reaped and final PTY output is drained and parsed.
//! Worker teardown is measured separately. A simulated 120 Hz consumer clears
//! pending redraw notifications; callbacks only increment an atomic counter.
//! This benchmark excludes a GUI, shaping, GPU work, and input latency.

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    unix::run()
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This benchmark currently requires Unix with /bin/sh, stty, and awk.");
    std::process::exit(2);
}

#[cfg(unix)]
mod unix {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use anyhow::{Result, bail, ensure};
    use terminal_core::{SessionOptions, SessionStatus, TerminalSession};

    const PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;
    const LINE_BYTES: u64 = 128;
    const LINE_COUNT: u64 = PAYLOAD_BYTES / LINE_BYTES;
    const READY: &[u8] = b"\x1b]0;pty-pipeline-ready\x07";
    const FINAL_MARKER: &str = "FINAL-PTY-PIPELINE-FRAME";
    const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 120);

    // Passing line bodies as argv avoids shell interpolation. Raw mode disables
    // echo and output newline translation, making the emitted byte count exact.
    const SCRIPT: &str = r#"
stty raw -echo || exit 2
printf '\033]0;pty-pipeline-ready\007' || exit 3
IFS= read -r start || exit 4
exec awk -v line="$1" -v final="$2" -v count="$3" '
BEGIN {
    for (i = 0; i < count - 1; i++) printf "%s\r\n", line
    printf "%s\r\n", final
}'
"#;

    pub fn run() -> Result<()> {
        let repaint_count = Arc::new(AtomicU64::new(0));
        let callback_count = repaint_count.clone();
        let spawn_started = Instant::now();
        let session = TerminalSession::spawn(
            SessionOptions {
                shell: Some("/bin/sh".into()),
                args: vec![
                    "-c".into(),
                    SCRIPT.into(),
                    "pty-pipeline-bench".into(),
                    padded(
                        "2026-09-30T12:04:38.125Z INFO worker=042 task=4821 completed successfully duration=0.83ms",
                    ),
                    padded(FINAL_MARKER),
                    LINE_COUNT.to_string(),
                ],
                env: vec![("LC_ALL".into(), "C".into())],
                cols: 160,
                rows: 40,
                scrollback: 10_000,
                ..SessionOptions::default()
            },
            Arc::new(move || {
                callback_count.fetch_add(1, Ordering::Relaxed);
            }),
        )?;
        let ready_deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let metadata = session.metadata();
            if metadata.title == "pty-pipeline-ready"
                && session.metrics().bytes_parsed >= READY.len() as u64
                && repaint_count.load(Ordering::Relaxed) > 0
            {
                break;
            }
            ensure!(
                metadata.status == SessionStatus::Running,
                "Producer failed before ready: {:?}",
                metadata.status
            );
            ensure!(
                Instant::now() < ready_deadline,
                "Producer ready handshake timed out"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let ready_duration = spawn_started.elapsed();
        let before = session.metrics();
        ensure!(
            before.bytes_received == READY.len() as u64
                && before.bytes_parsed == READY.len() as u64,
            "Unexpected setup output: received={}, parsed={}",
            before.bytes_received,
            before.bytes_parsed
        );
        let callbacks_before = repaint_count.load(Ordering::Relaxed);
        session.acknowledge_repaint();

        let started = Instant::now();
        let mut last_frame = started;
        session.write(b"start\n")?;
        let output_deadline = started + Duration::from_secs(120);
        loop {
            if last_frame.elapsed() >= FRAME_INTERVAL {
                // Match the renderer contract: acknowledge before any snapshot
                // read. This models notification consumption, not frame work.
                session.acknowledge_repaint();
                last_frame = Instant::now();
            }
            match session.metadata().status {
                SessionStatus::Running => {}
                SessionStatus::Exited {
                    code: 0,
                    signal: None,
                } => break,
                status => bail!("Producer did not exit successfully: {status:?}"),
            }
            ensure!(Instant::now() < output_deadline, "PTY pipeline timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
        // Exited is published after EOF and after the final parser flush.
        let duration = started.elapsed();
        let cleanup_started = Instant::now();
        while session.metrics().active_workers != 0 {
            ensure!(
                cleanup_started.elapsed() < Duration::from_secs(5),
                "PTY workers did not stop"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let cleanup_duration = cleanup_started.elapsed();
        let after = session.metrics();
        let received = after.bytes_received - before.bytes_received;
        let parsed = after.bytes_parsed - before.bytes_parsed;
        ensure!(
            received == PAYLOAD_BYTES && parsed == PAYLOAD_BYTES,
            "Expected {PAYLOAD_BYTES} payload bytes; received={received}, parsed={parsed}"
        );
        ensure!(session.history_size() == 10_000, "Unexpected history size");
        let visible = session.screen_text();
        ensure!(
            visible.contains(FINAL_MARKER),
            "The final producer frame was not parsed"
        );

        let mib = PAYLOAD_BYTES as f64 / (1024.0 * 1024.0);
        println!("Fixture: {LINE_COUNT} CRLF lines × {LINE_BYTES} bytes = {mib:.2} MiB");
        println!(
            "Spawn to ready: {:.3} ms (excluded)",
            ready_duration.as_secs_f64() * 1000.0
        );
        println!(
            "PTY pipeline: {mib:.2} MiB in {:.3} s = {:.1} MiB/s",
            duration.as_secs_f64(),
            mib / duration.as_secs_f64()
        );
        println!("Payload bytes received / parsed: {received} / {parsed} (verified)");
        println!(
            "VT parsing timed sections: {:.3} s (excludes grid mutex wait)",
            (after.parse_nanoseconds - before.parse_nanoseconds) as f64 / 1e9
        );
        println!(
            "Repaint callbacks (simulated 120 Hz consumer): {}; VT revisions: {}; history: 10000 rows",
            repaint_count.load(Ordering::Relaxed) - callbacks_before,
            after.revision - before.revision
        );
        println!(
            "Worker cleanup: {:.3} ms (excluded)",
            cleanup_duration.as_secs_f64() * 1000.0
        );
        #[cfg(target_os = "linux")]
        if let Ok(status) = std::fs::read_to_string("/proc/self/status")
            && let Some(line) = status.lines().find(|line| line.starts_with("VmHWM:"))
        {
            println!(
                "Process peak resident memory: {}",
                line.trim_start_matches("VmHWM:").trim()
            );
        }
        Ok(())
    }

    fn padded(body: &str) -> String {
        // Every body is ASCII and leaves two bytes for its literal CRLF.
        assert!(body.is_ascii() && body.len() <= LINE_BYTES as usize - 2);
        format!("{body:<width$}", width = LINE_BYTES as usize - 2)
    }
}
