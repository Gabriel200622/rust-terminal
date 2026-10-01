//! Manual Unix regression with the real user's zsh startup files. Reports only
//! geometry and assertions; shell configuration and terminal text stay private.

use std::sync::Arc;
use std::time::Duration;

use terminal_core::{SessionOptions, TerminalSession};

fn main() -> anyhow::Result<()> {
    let session = TerminalSession::spawn(
        SessionOptions {
            cwd: "/tmp".into(),
            shell: Some("zsh".into()),
            args: vec!["-i".into()],
            cols: 100,
            rows: 30,
            ..SessionOptions::default()
        },
        Arc::new(|| {}),
    )?;
    std::thread::sleep(Duration::from_millis(1200));
    session.write(b"printf 'RESIZE-COMMAND-READY\\n'\r")?;
    std::thread::sleep(Duration::from_millis(400));
    check(&session, "before")?;
    for cols in [80, 50, 33, 50, 80, 100] {
        session.resize(cols, 30, 0, 0)?;
        std::thread::sleep(Duration::from_millis(300));
        check(&session, &format!("after {cols}"))?;
    }
    session.shutdown();
    Ok(())
}

fn check(session: &TerminalSession, label: &str) -> anyhow::Result<()> {
    let terminal = session.viewport();
    let rows: Vec<String> = terminal
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.c).collect())
        .collect();
    let command_end = rows
        .iter()
        .position(|row| row.trim() == "RESIZE-COMMAND-READY");
    anyhow::ensure!(command_end.is_some(), "Command output was lost at {label}");
    let prompt = &rows[command_end.unwrap() + 1..];
    anyhow::ensure!(
        prompt.iter().filter(|row| !row.trim().is_empty()).count() == 2,
        "Duplicated or stale prompt rows at {label}"
    );
    anyhow::ensure!(
        prompt
            .iter()
            .map(|row| row.matches("/tmp").count())
            .sum::<usize>()
            == 1,
        "Expected one live directory prompt at {label}"
    );
    println!(
        "{label}: {}x{}; cursor={:?}; command output retained; exactly two live prompt rows, one directory (verified)",
        terminal.columns, terminal.screen_lines, terminal.cursor.point
    );
    Ok(())
}
