//! Scoped CLI adapters and a bounded, metadata-only local hook bridge.
//! All setup and socket/file I/O runs on startup workers or the bridge worker.
use neptune_model::{AgentKind, AgentSession, PaneId};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{IsTerminal, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use terminal_core::SessionOptions;

type Wake = Arc<dyn Fn() + Send + Sync>;
const MAX_MESSAGE: u64 = 40 * 1024;
const ENDPOINT: &str = "NEPTUNE_AGENT_ENDPOINT";
const TOKEN: &str = "NEPTUNE_AGENT_TOKEN";
const SHIMS: &str = "NEPTUNE_AGENT_SHIMS";
const RUN: &str = "NEPTUNE_AGENT_RUN";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    Open { agent: AgentSession },
    Session { agent: AgentSession },
    Close,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    token: String,
    run: String,
    event: Event,
}
struct Slot {
    generation: u64,
    token: String,
    run: Option<String>,
    wake: Wake,
    _startup: Option<tempfile::TempDir>,
}
#[derive(Default)]
struct Shared {
    slots: BTreeMap<PaneId, Slot>,
    changes: BTreeMap<PaneId, (u64, Option<AgentSession>)>,
    retired: Vec<tempfile::TempDir>,
}
struct Listener {
    directory: tempfile::TempDir,
    address: String,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(&self.address);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[derive(Default)]
pub struct AgentBridge {
    listener: Mutex<Option<Listener>>,
    shared: Arc<Mutex<Shared>>,
}
impl AgentBridge {
    /// Called only by a startup worker. Tests and remote sessions opt out.
    pub fn prepare(
        &self,
        pane: PaneId,
        generation: u64,
        options: &mut SessionOptions,
        resume: Option<&AgentSession>,
        wake: Wake,
    ) -> std::io::Result<()> {
        #[cfg(not(unix))]
        {
            let _ = (pane, generation, options, resume, wake);
            return Ok(());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut state = self
                .listener
                .lock()
                .map_err(|_| std::io::Error::other("Agent bridge unavailable"))?;
            if state.is_none() {
                let directory = tempfile::Builder::new()
                    .prefix("neptune-agents-")
                    .tempdir()?;
                let executable = std::env::current_exe()?;
                for kind in [AgentKind::Claude, AgentKind::Codex] {
                    let path = directory.path().join(kind.executable());
                    std::fs::write(
                        &path,
                        format!(
                            "#!/bin/sh\nexec {} --agent-run {} \"$@\"\n",
                            quote(&executable.to_string_lossy()),
                            kind.executable()
                        ),
                    )?;
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
                }
                let socket = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
                let address = socket.local_addr()?.to_string();
                let stop = Arc::new(AtomicBool::new(false));
                let stopping = stop.clone();
                let shared = self.shared.clone();
                let worker = thread::Builder::new()
                    .name("neptune-agent-hooks".into())
                    .spawn(move || {
                        for stream in socket.incoming() {
                            if stopping.load(Ordering::Acquire) {
                                break;
                            }
                            let Ok(mut stream) = stream else { break };
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
                            let mut bytes = Vec::new();
                            if read_message(&mut stream, &mut bytes).is_err() {
                                continue;
                            }
                            if let Ok(message) = serde_json::from_slice::<Message>(&bytes) {
                                apply_message(&shared, message);
                            }
                            let _ = stream.write_all(b"ok");
                        }
                    })?;
                *state = Some(Listener {
                    directory,
                    address,
                    stop,
                    worker: Some(worker),
                });
            }
            let listener = state
                .as_ref()
                .ok_or_else(|| std::io::Error::other("Agent bridge unavailable"))?;
            let nonce = tempfile::Builder::new()
                .rand_bytes(32)
                .tempfile_in(listener.directory.path())?;
            let token = nonce
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let shim_path = listener.directory.path().to_string_lossy().into_owned();
            let startup = tempfile::Builder::new()
                .prefix("pane-")
                .tempdir_in(listener.directory.path())?;
            let path = option_env(options, "PATH").unwrap_or_default();
            options.env.extend([
                (ENDPOINT.into(), listener.address.clone()),
                (TOKEN.into(), token.clone()),
                (SHIMS.into(), shim_path.clone()),
                (RUN.into(), String::new()),
                ("PATH".into(), format!("{shim_path}:{path}")),
            ]);
            configure_shell(options, startup.path(), &std::env::current_exe()?, resume)?;
            let (old, retired) = {
                let mut shared = self
                    .shared
                    .lock()
                    .map_err(|_| std::io::Error::other("Agent bridge unavailable"))?;
                shared.changes.remove(&pane);
                let old = shared.slots.insert(
                    pane,
                    Slot {
                        generation,
                        token,
                        run: None,
                        wake,
                        _startup: Some(startup),
                    },
                );
                (old, std::mem::take(&mut shared.retired))
            };
            drop((old, retired));
            Ok(())
        }
    }
    pub fn close(&self, pane: PaneId) {
        if let Ok(mut shared) = self.shared.lock() {
            if let Some(mut slot) = shared.slots.remove(&pane)
                && let Some(startup) = slot._startup.take()
            {
                shared.retired.push(startup);
            }
            shared.changes.remove(&pane);
        }
    }
    pub fn close_generation(&self, pane: PaneId, generation: u64) {
        if let Ok(mut shared) = self.shared.lock()
            && shared
                .slots
                .get(&pane)
                .is_some_and(|slot| slot.generation == generation)
        {
            if let Some(mut slot) = shared.slots.remove(&pane)
                && let Some(startup) = slot._startup.take()
            {
                shared.retired.push(startup);
            }
            shared.changes.remove(&pane);
        }
    }
    pub fn drain(&self) -> Vec<(PaneId, u64, Option<AgentSession>)> {
        self.shared
            .lock()
            .map(|mut shared| {
                std::mem::take(&mut shared.changes)
                    .into_iter()
                    .map(|(pane, (generation, agent))| (pane, generation, agent))
                    .collect()
            })
            .unwrap_or_default()
    }
}
#[cfg(unix)]
fn configure_shell(
    options: &mut SessionOptions,
    directory: &std::path::Path,
    helper: &std::path::Path,
    resume: Option<&AgentSession>,
) -> std::io::Result<()> {
    let default_shell = options.shell.is_none();
    // Configured arguments are the user's: start the shell as given instead
    // of replacing them with Neptune's zsh/bash startup.
    let configured_args = !options.args.is_empty();
    let shell = options
        .shell
        .clone()
        .or_else(|| option_env(options, "SHELL"))
        .unwrap_or_else(|| "/bin/sh".into());
    let name = std::path::Path::new(&shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let resume_json = resume
        .filter(|agent| agent.is_valid())
        .map(serde_json::to_string)
        .transpose()?;
    let mut setup = "export PATH=\"$NEPTUNE_AGENT_SHIMS:$PATH\"\n".to_owned();
    if let Some(resume) = &resume_json {
        if name == "zsh" {
            // Powerlevel10k's instant prompt keeps stdio off the terminal until the
            // first prompt, later than any startup file; the agent needs it now.
            setup.push_str("(( ${+functions[p10k]} )) && p10k clear-instant-prompt\n");
        }
        // Quoted literal data, never interpolated terminal input. This runs after user startup files.
        setup.push_str(&format!(
            "{} --agent-restore {}\n",
            quote(&helper.to_string_lossy()),
            quote(resume)
        ));
    }
    match name {
        "zsh" if !configured_args => {
            let dotdir = tempfile::Builder::new()
                .prefix("zsh-")
                .tempdir_in(directory)?
                .keep();
            let user_dir = option_env(options, "ZDOTDIR");
            options.env.push((
                "NEPTUNE_USER_ZDOTDIR".into(),
                user_dir
                    .clone()
                    .unwrap_or_else(|| option_env(options, "HOME").unwrap_or_default()),
            ));
            options.env.push((
                "NEPTUNE_USER_ZDOTDIR_SET".into(),
                if user_dir.is_some() { "1" } else { "0" }.into(),
            ));
            let restore = "if [[ $NEPTUNE_USER_ZDOTDIR_SET == 1 ]]; then ZDOTDIR=$NEPTUNE_USER_ZDOTDIR; else unset ZDOTDIR; fi\n";
            let quoted_dotdir = quote(&dotdir.to_string_lossy());
            let capture = format!(
                "NEPTUNE_USER_ZDOTDIR=${{ZDOTDIR-$HOME}}\nNEPTUNE_USER_ZDOTDIR_SET=${{+ZDOTDIR}}\nZDOTDIR={quoted_dotdir}\n"
            );
            // Global zshrc (macOS /etc/zshrc) runs while this directory is
            // ZDOTDIR and puts HISTFILE in it; keep history in the user's ZDOTDIR.
            let history = format!(
                "[[ $HISTFILE == {quoted_dotdir}/* ]] && HISTFILE=${{ZDOTDIR-$HOME}}/${{HISTFILE#{quoted_dotdir}/}}\n"
            );
            for file in [".zshenv", ".zprofile", ".zshrc", ".zlogin"] {
                let mut content = restore.to_owned();
                if file == ".zshrc" {
                    content.push_str(&history);
                }
                content.push_str(&format!(
                    "[[ -r ${{ZDOTDIR-$HOME}}/{file} ]] && source \"${{ZDOTDIR-$HOME}}/{file}\"\n"
                ));
                match file {
                    ".zshrc" => content.push_str(&format!("if [[ ! -o login ]]; then\n{setup}unset NEPTUNE_USER_ZDOTDIR NEPTUNE_USER_ZDOTDIR_SET\nelse\n{capture}fi\n")),
                    ".zlogin" => content.push_str(&format!("{setup}unset NEPTUNE_USER_ZDOTDIR NEPTUNE_USER_ZDOTDIR_SET\n")),
                    _ => content.push_str(&capture),
                }
                std::fs::write(dotdir.join(file), content)?;
            }
            options
                .env
                .push(("ZDOTDIR".into(), dotdir.to_string_lossy().into_owned()));
            options.shell = Some(shell);
            options.args = vec![if default_shell { "-il" } else { "-i" }.into()];
        }
        "bash" if !configured_args => {
            let mut file = tempfile::Builder::new()
                .prefix("bash-")
                .tempfile_in(directory)?;
            let startup = if default_shell {
                "[[ -r /etc/profile ]] && source /etc/profile\nfor f in ~/.bash_profile ~/.bash_login ~/.profile; do if [[ -r $f ]]; then source \"$f\"; break; fi; done\nunset f\n"
            } else {
                "[[ -r ~/.bashrc ]] && source ~/.bashrc\n"
            };
            file.write_all(format!("{startup}{setup}").as_bytes())?;
            let (_, path) = file.keep().map_err(|error| error.error)?;
            options.shell = Some(shell);
            options.args = vec![
                "--rcfile".into(),
                path.to_string_lossy().into_owned(),
                "-i".into(),
            ];
        }
        _ => {
            if let Some(resume) = resume_json {
                options.shell = Some("/bin/sh".into());
                let args = std::mem::take(&mut options.args);
                options.args = vec![
                    "-c".into(),
                    "\"$1\" --agent-restore \"$2\"; shift 2; exec \"$@\"".into(),
                    "neptune".into(),
                    helper.to_string_lossy().into_owned(),
                    resume,
                    shell,
                ];
                options.args.extend(args);
            }
        }
    }
    Ok(())
}
fn read_message(stream: &mut TcpStream, bytes: &mut Vec<u8>) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_millis(200);
    let mut buffer = [0; 4096];
    loop {
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::TimedOut, "Agent event timed out")
            })?;
        stream.set_read_timeout(Some(remaining))?;
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        if bytes.len() + count > MAX_MESSAGE as usize {
            return Err(std::io::Error::other("Agent event too large"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}
fn option_env(options: &SessionOptions, key: &str) -> Option<String> {
    options
        .env
        .iter()
        .rev()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var(key).ok())
}
fn apply_message(shared: &Mutex<Shared>, message: Message) {
    let Ok(mut shared) = shared.lock() else {
        return;
    };
    let Some((&pane, slot)) = shared
        .slots
        .iter_mut()
        .find(|(_, slot)| slot.token == message.token)
    else {
        return;
    };
    let agent = match message.event {
        Event::Open { agent } if agent.is_valid() => {
            slot.run = Some(message.run);
            Some(agent)
        }
        Event::Session { agent } if slot.run.as_ref() == Some(&message.run) && agent.is_valid() => {
            Some(agent)
        }
        Event::Close if slot.run.as_ref() == Some(&message.run) => {
            slot.run = None;
            None
        }
        _ => return,
    };
    let generation = slot.generation;
    let wake = slot.wake.clone();
    shared.changes.insert(pane, (generation, agent));
    drop(shared);
    wake();
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn kind(value: &str) -> anyhow::Result<AgentKind> {
    match value {
        "claude" => Ok(AgentKind::Claude),
        "codex" => Ok(AgentKind::Codex),
        _ => anyhow::bail!("Unknown agent"),
    }
}
fn send(event: Event, run: &str) -> anyhow::Result<()> {
    let endpoint: std::net::SocketAddr = std::env::var(ENDPOINT)?.parse()?;
    anyhow::ensure!(endpoint.ip().is_loopback(), "Invalid agent bridge address");
    let message = Message {
        token: std::env::var(TOKEN)?,
        run: run.into(),
        event,
    };
    let mut stream = TcpStream::connect_timeout(&endpoint, Duration::from_millis(200))?;
    stream.set_write_timeout(Some(Duration::from_millis(200)))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.write_all(&serde_json::to_vec(&message)?)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut ack = [0; 2];
    stream.read_exact(&mut ack)?;
    Ok(())
}
/// Private CLI entry points, handled before desktop initialization. Never logs hook input.
pub fn cli(args: &[String]) -> anyhow::Result<Option<i32>> {
    match args.first().map(String::as_str) {
        Some("--agent-hook") => {
            let provider = kind(args.get(1).map(String::as_str).unwrap_or_default())?;
            #[derive(Deserialize)]
            struct Hook {
                session_id: String,
                cwd: PathBuf,
                hook_event_name: String,
            }
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(MAX_MESSAGE + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 <= MAX_MESSAGE
                && let Ok(hook) = serde_json::from_slice::<Hook>(&bytes)
            {
                let agent = AgentSession {
                    kind: provider,
                    session_id: Some(hook.session_id),
                    cwd: hook.cwd,
                };
                if hook.hook_event_name == "SessionStart" && agent.is_valid() {
                    let _ = send(
                        Event::Session { agent },
                        &std::env::var(RUN).unwrap_or_default(),
                    );
                }
            }
            Ok(Some(0))
        }
        Some("--agent-close") => {
            let failed_resume = args.get(1).is_some_and(|status| status != "0")
                && args.get(2).is_some_and(|resumed| resumed == "1");
            if failed_resume {
                eprintln!(
                    "Neptune: resume failed; the session reference is kept. Restart this terminal for a fresh shell."
                );
            } else {
                let _ = send(Event::Close, &std::env::var(RUN).unwrap_or_default());
            }
            Ok(Some(0))
        }
        Some("--agent-run") => {
            let provider = kind(args.get(1).map(String::as_str).unwrap_or_default())?;
            Ok(Some(run_agent(provider, &args[2..], None)?))
        }
        Some("--agent-restore") => {
            let agent: AgentSession = serde_json::from_str(
                args.get(1)
                    .ok_or_else(|| anyhow::anyhow!("Missing resume reference"))?,
            )?;
            anyhow::ensure!(agent.is_valid(), "Invalid resume reference");
            // Startup files can leave stdio redirected. Without the terminal the CLI
            // would run as a batch job; keep the reference and leave the shell quiet.
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Ok(Some(0));
            }
            let arguments = match &agent.session_id {
                Some(id) => vec![
                    match agent.kind {
                        AgentKind::Claude => "--resume",
                        AgentKind::Codex => "resume",
                    }
                    .into(),
                    id.clone(),
                ],
                None => Vec::new(),
            };
            Ok(Some(run_agent(agent.kind, &arguments, Some(&agent))?))
        }
        _ => Ok(None),
    }
}
fn resolve(provider: AgentKind) -> anyhow::Result<PathBuf> {
    let shims = std::env::var_os(SHIMS).map(PathBuf::from);
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        if shims.as_ref() == Some(&directory) {
            continue;
        }
        let candidate = directory.join(provider.executable());
        if candidate.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if candidate.metadata()?.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }
            return Ok(candidate);
        }
    }
    anyhow::bail!(
        "{} is not installed or is not on PATH",
        provider.executable()
    )
}
fn interactive(provider: AgentKind, args: &[String]) -> bool {
    if args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--help" | "-h" | "--version" | "-V" | "--remote" | "--remote-control"
        )
    }) {
        return false;
    }
    if provider == AgentKind::Claude && args.iter().any(|arg| arg == "-p" || arg == "--print") {
        return false;
    }
    let commands: &[&str] = match provider {
        AgentKind::Claude => &[
            "auth",
            "mcp",
            "plugin",
            "install",
            "update",
            "doctor",
            "setup-token",
            "agents",
            "remote-control",
        ],
        AgentKind::Codex => &[
            "exec",
            "e",
            "review",
            "login",
            "logout",
            "mcp",
            "plugin",
            "app-server",
            "remote-control",
            "completion",
            "update",
            "doctor",
            "sandbox",
            "debug",
            "apply",
            "queue",
            "archive",
            "delete",
            "migrate-rollouts",
            "unarchive",
            "cloud",
            "exec-server",
            "features",
            "help",
            "agents",
        ],
    };
    // Conservative around option-led subcommands too: never relaunch a batch job.
    !args.iter().any(|arg| commands.contains(&arg.as_str()))
}
fn run_agent(
    provider: AgentKind,
    args: &[String],
    resumed: Option<&AgentSession>,
) -> anyhow::Result<i32> {
    let executable = resolve(provider)?;
    let mut command = std::process::Command::new(executable);
    let nested = std::env::var(RUN).is_ok_and(|value| !value.is_empty());
    if nested
        || !interactive(provider, args)
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
        || std::env::var(ENDPOINT).is_err()
    {
        command.args(args);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            return Err(command.exec().into());
        }
        #[cfg(not(unix))]
        {
            return Ok(command.status()?.code().unwrap_or(1));
        }
    }
    let invocation = tempfile::Builder::new()
        .prefix("neptune-agent-run-")
        .rand_bytes(32)
        .tempfile()?;
    let run = invocation
        .path()
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    drop(invocation);
    let agent = resumed.cloned().unwrap_or(AgentSession {
        kind: provider,
        session_id: None,
        cwd: std::env::current_dir()?,
    });
    if send(Event::Open { agent }, &run).is_err() {
        eprintln!("Neptune: agent session tracking is unavailable for this launch.");
    }
    let hook = format!(
        "{} --agent-hook {}",
        quote(&std::env::current_exe()?.to_string_lossy()),
        provider.executable()
    );
    match provider {
        AgentKind::Claude => {
            let settings = serde_json::json!({"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":hook}]}]}});
            command.args(["--settings", &settings.to_string()]);
        }
        AgentKind::Codex => {
            let hook = toml::Value::String(hook).to_string();
            // Hook processes need this terminal's environment, not a shared daemon's.
            // Older Codex versions still open normally; they can restore the CLI only.
            let supports_local = std::process::Command::new(resolve(provider)?)
                .arg("--help")
                .output()
                .is_ok_and(|output| {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout).contains("--no-daemon")
                });
            if supports_local {
                command.args([
                    "--no-daemon",
                    "-c",
                    &format!(
                        "hooks.SessionStart=[{{hooks=[{{type=\"command\",command={hook}}}]}}]"
                    ),
                ]);
                // Codex excludes *TOKEN* from the default tool/hook environment.
                // Supply only this pane's bridge metadata as invocation-scoped overrides;
                // leave the user's other environment filters and hook trust intact.
                for (name, value) in [
                    (TOKEN, std::env::var(TOKEN).unwrap_or_default()),
                    (ENDPOINT, std::env::var(ENDPOINT).unwrap_or_default()),
                    (RUN, run.clone()),
                ] {
                    command.args([
                        "-c",
                        &format!(
                            "shell_environment_policy.set.{name}={}",
                            toml::Value::String(value)
                        ),
                    ]);
                }
            }
        }
    }
    command.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // The supervisor catches terminal interrupts while its child retains normal
        // signal handling. A Rust status() parent would die on Ctrl+C even when the
        // interactive agent handles it, leaving a second foreground reader behind.
        let mut supervisor = std::process::Command::new("/bin/sh");
        supervisor.args(["-c", "helper=$1; resumed=$2; shift 2; trap ':' INT QUIT; \"$@\"; result=$?; \"$helper\" --agent-close \"$result\" \"$resumed\"; exit \"$result\"", "neptune-agent"])
            .arg(std::env::current_exe()?).arg(if resumed.is_some() { "1" } else { "0" })
            .arg(command.get_program()).args(command.get_args()).env(RUN, &run);
        if let Some(agent) = resumed {
            supervisor.current_dir(&agent.cwd);
        }
        Err(supervisor.exec().into())
    }
    #[cfg(not(unix))]
    {
        if let Some(agent) = resumed {
            command.current_dir(&agent.cwd);
        }
        let result = command.env(RUN, &run).status();
        let _ = send(Event::Close, &run);
        Ok(result?.code().unwrap_or(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn agent(id: &str) -> AgentSession {
        AgentSession {
            kind: AgentKind::Claude,
            session_id: Some(id.into()),
            cwd: std::env::temp_dir(),
        }
    }
    const FIRST: &str = "019a1234-5678-7000-8000-123456789abc";
    const SECOND: &str = "019a1234-5678-7000-8000-123456789def";
    #[test]
    fn hooks_are_bound_to_pane_generation_and_invocation_and_coalesced() {
        let bridge = AgentBridge::default();
        for (id, token) in [(1, "one"), (2, "two")] {
            bridge.shared.lock().unwrap().slots.insert(
                PaneId::new(id),
                Slot {
                    generation: 7,
                    token: token.into(),
                    run: None,
                    wake: Arc::new(|| {}),
                    _startup: None,
                },
            );
        }
        let emit = |token: &str, run: &str, event| {
            apply_message(
                &bridge.shared,
                Message {
                    token: token.into(),
                    run: run.into(),
                    event,
                },
            )
        };
        emit(
            "one",
            "a",
            Event::Open {
                agent: agent(FIRST),
            },
        );
        emit(
            "two",
            "b",
            Event::Open {
                agent: agent(SECOND),
            },
        );
        emit(
            "one",
            "a",
            Event::Session {
                agent: agent(SECOND),
            },
        );
        let changes = bridge.drain();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0], (PaneId::new(1), 7, Some(agent(SECOND))));
        emit("wrong", "a", Event::Close);
        emit("one", "old", Event::Close);
        assert!(bridge.drain().is_empty());
        emit("one", "a", Event::Close);
        emit(
            "one",
            "a",
            Event::Session {
                agent: agent(FIRST),
            },
        );
        assert_eq!(bridge.drain(), vec![(PaneId::new(1), 7, None)]);
        bridge.close_generation(PaneId::new(2), 6);
        emit(
            "two",
            "b",
            Event::Session {
                agent: agent(FIRST),
            },
        );
        assert_eq!(
            bridge.drain(),
            vec![(PaneId::new(2), 7, Some(agent(FIRST)))]
        );
        bridge.close_generation(PaneId::new(2), 7);
        emit(
            "two",
            "b",
            Event::Session {
                agent: agent(FIRST),
            },
        );
        assert!(bridge.drain().is_empty());
    }
    #[test]
    fn batch_and_administrative_invocations_are_not_restored() {
        for args in [vec!["auth"], vec!["mcp"], vec!["--print"], vec!["--help"]] {
            assert!(!interactive(
                AgentKind::Claude,
                &args.into_iter().map(String::from).collect::<Vec<_>>()
            ));
        }
        assert!(interactive(AgentKind::Codex, &[]));
        assert!(interactive(
            AgentKind::Codex,
            &["resume".into(), FIRST.into()]
        ));
    }
    #[cfg(unix)]
    #[test]
    fn shell_startup_preserves_user_configuration_and_places_scoped_adapters_first() {
        use std::sync::Arc;
        use terminal_core::TerminalSession;
        for shell in ["/bin/bash", "/bin/zsh"] {
            if !std::path::Path::new(shell).is_file() {
                continue;
            }
            let root = tempfile::tempdir().unwrap();
            // Exercise the fixture's user config without host-wide interactive
            // setup (CI's compinit can prompt before the test command is read).
            std::fs::write(root.path().join(".zshenv"), "unsetopt GLOBAL_RCS\n").unwrap();
            std::fs::write(
                root.path().join(".bashrc"),
                "export PATH=/usr/bin:/bin\nexport NEPTUNE_USER_CONFIG=loaded\n",
            )
            .unwrap();
            std::fs::write(
                root.path().join(".zshrc"),
                "export PATH=/usr/bin:/bin\nexport NEPTUNE_USER_CONFIG=loaded\n",
            )
            .unwrap();
            let bridge = AgentBridge::default();
            let mut options = SessionOptions {
                shell: Some(shell.into()),
                cwd: root.path().into(),
                env: vec![
                    ("HOME".into(), root.path().to_string_lossy().into_owned()),
                    ("ZDOTDIR".into(), root.path().to_string_lossy().into_owned()),
                ],
                ..Default::default()
            };
            bridge
                .prepare(PaneId::new(1), 1, &mut options, None, Arc::new(|| {}))
                .unwrap();
            let session = TerminalSession::spawn(options, Arc::new(|| {})).unwrap();
            session.write(b"printf 'CONFIG=%s ADAPTER=%s\\n' \"$NEPTUNE_USER_CONFIG\" \"$(command -v claude)\"\r").unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                if session.screen_text().contains("CONFIG=loaded ADAPTER=/") {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "shell {shell} startup failed: {}",
                    session.screen_text()
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert!(session.screen_text().contains("neptune-agents-"));
            session.shutdown();
        }
    }
    #[cfg(unix)]
    #[test]
    fn zsh_history_stays_in_the_user_directory() {
        use std::sync::Arc;
        use terminal_core::TerminalSession;
        if !std::path::Path::new("/bin/zsh").is_file() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        std::fs::create_dir(&home).unwrap();
        std::fs::write(home.join(".zshenv"), "unsetopt GLOBAL_RCS\n").unwrap();
        // Stand in for macOS /etc/zshrc, which sets HISTFILE from the startup
        // ZDOTDIR before the user's .zshrc.
        std::fs::write(
            home.join(".zprofile"),
            "HISTFILE=$FIXTURE_STARTUP_DIR/.zsh_history\n",
        )
        .unwrap();
        std::fs::write(home.join(".zshrc"), "PROMPT='NEPTUNE> '\n").unwrap();
        let bridge = AgentBridge::default();
        let mut options = SessionOptions {
            cwd: home.clone(),
            env: vec![
                ("HOME".into(), home.to_string_lossy().into_owned()),
                ("SHELL".into(), "/bin/zsh".into()),
            ],
            ..Default::default()
        };
        bridge
            .prepare(PaneId::new(1), 1, &mut options, None, Arc::new(|| {}))
            .unwrap();
        let startup = option_env(&options, "ZDOTDIR").unwrap();
        options.env.push(("FIXTURE_STARTUP_DIR".into(), startup));
        let session = TerminalSession::spawn(options, Arc::new(|| {})).unwrap();
        session
            .write(b"print -r -- \"$HISTFILE\" > \"$HOME/histfile\"\r")
            .unwrap();
        let output = home.join("histfile");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !std::fs::read_to_string(&output).is_ok_and(|text| text.ends_with('\n')) {
            assert!(
                std::time::Instant::now() < deadline,
                "zsh startup failed: {}",
                session.screen_text()
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            std::fs::read_to_string(&output).unwrap(),
            format!("{}\n", home.join(".zsh_history").display())
        );
        session.shutdown();
    }
    #[cfg(unix)]
    #[test]
    fn zsh_restore_gets_the_terminal_back_from_instant_prompt() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Arc;
        use terminal_core::TerminalSession;
        if !std::path::Path::new("/bin/zsh").is_file() {
            return;
        }
        for login in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let home = root.path().join("home");
            std::fs::create_dir(&home).unwrap();
            std::fs::write(home.join(".zshenv"), "unsetopt GLOBAL_RCS\n").unwrap();
            // Stand in for Powerlevel10k: stdio is redirected for the rest of startup
            // and returned only by its clear-instant-prompt command.
            std::fs::write(
                home.join(".zshrc"),
                "exec {fd0}<&0 {fd1}>&1 {fd2}>&2 0</dev/null 1>/dev/null 2>&1\np10k() { [[ $# == 1 && $1 == clear-instant-prompt ]] || return 1; exec 0<&$fd0 1>&$fd1 2>&$fd2 }\n",
            )
            .unwrap();
            let helper = root.path().join("helper");
            std::fs::write(
                &helper,
                "#!/bin/sh\nstdio=redirected\n[ -t 0 ] && [ -t 1 ] && [ -t 2 ] && stdio=terminal\necho \"$stdio\" > \"$HOME/restore\"\n",
            )
            .unwrap();
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
            let mut options = SessionOptions {
                shell: (!login).then(|| "/bin/zsh".into()),
                cwd: home.clone(),
                env: vec![
                    ("HOME".into(), home.to_string_lossy().into_owned()),
                    ("ZDOTDIR".into(), home.to_string_lossy().into_owned()),
                    ("SHELL".into(), "/bin/zsh".into()),
                ],
                ..Default::default()
            };
            configure_shell(&mut options, root.path(), &helper, Some(&agent(FIRST))).unwrap();
            let session = TerminalSession::spawn(options, Arc::new(|| {})).unwrap();
            let output = home.join("restore");
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while !std::fs::read_to_string(&output).is_ok_and(|text| text.ends_with('\n')) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "zsh startup failed: {}",
                    session.screen_text()
                );
                thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(std::fs::read_to_string(&output).unwrap(), "terminal\n");
            session.shutdown();
        }
    }
    #[cfg(unix)]
    #[test]
    fn configured_shell_arguments_are_kept_and_follow_a_resume() {
        use std::sync::Arc;
        let bridge = AgentBridge::default();
        let mut options = SessionOptions {
            shell: Some("/bin/bash".into()),
            args: vec!["--norc".into(), "-i".into()],
            ..Default::default()
        };
        bridge
            .prepare(PaneId::new(1), 1, &mut options, None, Arc::new(|| {}))
            .unwrap();
        assert_eq!(options.shell.as_deref(), Some("/bin/bash"));
        assert_eq!(options.args, ["--norc", "-i"]);

        let mut options = SessionOptions {
            shell: Some("/bin/zsh".into()),
            args: vec!["-l".into()],
            ..Default::default()
        };
        bridge
            .prepare(
                PaneId::new(2),
                1,
                &mut options,
                Some(&agent(FIRST)),
                Arc::new(|| {}),
            )
            .unwrap();
        assert_eq!(options.shell.as_deref(), Some("/bin/sh"));
        // `shift 2` leaves the shell and its own arguments for `exec "$@"`.
        assert_eq!(options.args[5..], ["/bin/zsh", "-l"]);
    }
}
