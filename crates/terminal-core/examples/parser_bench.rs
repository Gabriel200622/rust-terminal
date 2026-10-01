//! `cargo run --release -p terminal-core --example parser_bench`
//!
//! A reproducible parser/scrollback benchmark, excluding PTY and GPU rendering.
//! Each case parses 64 MiB in 64 KiB batches with 10,000 history rows.

use std::time::Instant;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;

struct Size;
impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        40
    }
    fn screen_lines(&self) -> usize {
        40
    }
    fn columns(&self) -> usize {
        160
    }
}

fn run(name: &str, line: &str) {
    let payload = line.repeat(1024 * 1024 / line.len());
    let mut terminal = Term::new(Config::default(), &Size, VoidListener);
    let mut processor: Processor = Processor::new();
    processor.advance(&mut terminal, line.as_bytes());
    let repetitions = 64 * 1024 * 1024 / payload.len();
    let started = Instant::now();
    for _ in 0..repetitions {
        for chunk in payload.as_bytes().chunks(64 * 1024) {
            processor.advance(&mut terminal, chunk);
        }
    }
    let duration = started.elapsed();
    let mib = (repetitions * payload.len()) as f64 / (1024.0 * 1024.0);
    assert_eq!(terminal.history_size(), 10_000);
    println!(
        "{name}: {mib:.2} MiB, {:.3}s, {:.1} MiB/s, history={} rows",
        duration.as_secs_f64(),
        mib / duration.as_secs_f64(),
        terminal.history_size()
    );
}

fn main() {
    run(
        "plain log",
        "2026-09-30T12:04:38.125Z INFO worker::runtime task=4821 completed successfully duration=0.83ms\r\n",
    );
    run(
        "ANSI + Unicode",
        "\x1b[32m   Compiling\x1b[0m \x1b[1mterminal-core\x1b[0m v0.1.0 \x1b[38;2;143;171;217m✓\x1b[0m 界面 café e\u{301}\r\n",
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
}
