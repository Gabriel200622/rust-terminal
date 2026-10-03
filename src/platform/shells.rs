//! Shells offered in Preferences, found the way the platform's own terminal
//! finds them. Detection reads the filesystem, the registry and `vswhere`, so it
//! runs on a worker; frames only read the finished list.
use eframe::egui;
use std::sync::mpsc::{self, Receiver};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shell {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    /// What a new terminal runs without a configured shell. Choosing it saves
    /// no shell, so the platform's default startup (a Unix login shell) applies.
    pub default: bool,
}

impl Shell {
    fn new(name: impl Into<String>, program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            name: name.into(),
            program: program.into(),
            args: args.iter().map(|arg| (*arg).into()).collect(),
            default: false,
        }
    }
    /// Whether `program` and `args` start this shell.
    pub fn runs(&self, program: &str, args: &[String]) -> bool {
        same_program(&self.program, program) && self.args == args
    }
}

#[derive(Default)]
pub struct Detection {
    receiver: Option<Receiver<Vec<Shell>>>,
    shells: Option<Vec<Shell>>,
}

impl Detection {
    /// The detected shells, or `None` while detection runs. The first call
    /// starts it; a failed worker yields an empty list.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<&[Shell]> {
        if self.shells.is_none() {
            match &self.receiver {
                None => {
                    let (sender, receiver) = mpsc::sync_channel(1);
                    let wake = ctx.clone();
                    let started = std::thread::Builder::new()
                        .name("neptune-shells".into())
                        .spawn(move || {
                            let _ = sender.send(detect());
                            wake.request_repaint();
                        });
                    if started.is_ok() {
                        self.receiver = Some(receiver);
                    } else {
                        self.shells = Some(Vec::new());
                    }
                }
                Some(receiver) => match receiver.try_recv() {
                    Ok(shells) => self.shells = Some(shells),
                    Err(mpsc::TryRecvError::Disconnected) => self.shells = Some(Vec::new()),
                    Err(mpsc::TryRecvError::Empty) => {}
                },
            }
        }
        self.shells.as_deref()
    }
}

fn same_program(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Drops repeated commands, marks the default and tells apart equal names by
/// the folder of their program.
fn finish(mut shells: Vec<Shell>, default: Option<&str>) -> Vec<Shell> {
    let mut unique: Vec<Shell> = Vec::new();
    for shell in shells.drain(..) {
        if !unique
            .iter()
            .any(|seen| seen.runs(&shell.program, &shell.args))
        {
            unique.push(shell);
        }
    }
    if let Some(default) = default
        && let Some(shell) = unique
            .iter_mut()
            .find(|shell| shell.args.is_empty() && same_program(&shell.program, default))
    {
        shell.default = true;
    }
    let names: Vec<String> = unique.iter().map(|shell| shell.name.clone()).collect();
    for shell in &mut unique {
        if names.iter().filter(|name| **name == shell.name).count() > 1
            && let Some(folder) = std::path::Path::new(&shell.program).parent()
        {
            shell.name = format!("{} ({})", shell.name, folder.display());
        }
    }
    unique
}

#[cfg(unix)]
fn detect() -> Vec<Shell> {
    use std::path::PathBuf;
    let mut paths: Vec<PathBuf> = std::fs::read_to_string("/etc/shells")
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('/'))
        .map(PathBuf::from)
        .collect();
    // Shells installed by a package manager are often missing from
    // /etc/shells, and a GUI app's PATH can omit Homebrew.
    let mut folders: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    folders.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/home/linuxbrew/.linuxbrew/bin",
        ]
        .map(PathBuf::from),
    );
    for name in ["bash", "zsh", "fish", "nu", "pwsh", "xonsh", "elvish"] {
        if let Some(path) = folders
            .iter()
            .map(|folder| folder.join(name))
            .find(|path| path.is_file())
        {
            paths.push(path);
        }
    }
    // Merged /usr makes /bin/zsh and /usr/bin/zsh one shell; keep the first.
    let mut seen = Vec::new();
    let mut shells = Vec::new();
    let default = std::env::var("SHELL")
        .ok()
        .and_then(|shell| std::fs::canonicalize(shell).ok());
    let mut default_program = None;
    for path in paths {
        let Ok(real) = std::fs::canonicalize(&path) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        // git-shell only accepts git commands.
        if !real.is_file() || name.is_empty() || name == "git-shell" || seen.contains(&real) {
            continue;
        }
        let program = path.to_string_lossy().into_owned();
        if default.as_deref() == Some(real.as_path()) {
            default_program = Some(program.clone());
        }
        seen.push(real);
        shells.push(Shell::new(name, program, &[]));
    }
    finish(shells, default_program.as_deref())
}

#[cfg(windows)]
fn detect() -> Vec<Shell> {
    use std::path::PathBuf;
    let env = |name: &str| std::env::var(name).ok().map(PathBuf::from);
    // App execution aliases are reparse points that cannot be opened, so
    // existence is checked without following them.
    let exists = |path: &PathBuf| path.symlink_metadata().is_ok();
    let root = env("SystemRoot").unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let system = root.join("System32");
    let text = |path: &PathBuf| path.to_string_lossy().into_owned();
    let cmd = text(&system.join("cmd.exe"));
    let powershell = system.join(r"WindowsPowerShell\v1.0\powershell.exe");
    let mut shells = vec![Shell::new("Command Prompt", &cmd, &[])];
    if exists(&powershell) {
        shells.push(Shell::new("Windows PowerShell", text(&powershell), &[]));
    }

    let local = env("LOCALAPPDATA");
    let mut pwsh = Vec::new();
    for programs in [env("ProgramFiles"), env("ProgramFiles(x86)")]
        .into_iter()
        .flatten()
    {
        if let Ok(versions) = std::fs::read_dir(programs.join("PowerShell")) {
            let mut versions: Vec<PathBuf> = versions
                .flatten()
                .map(|entry| entry.path().join("pwsh.exe"))
                .collect();
            versions.sort();
            pwsh.extend(versions.into_iter().rev().filter(exists));
        }
    }
    if let Some(local) = &local {
        let apps = local.join(r"Microsoft\WindowsApps");
        pwsh.extend(
            [
                "Microsoft.PowerShell_8wekyb3d8bbwe",
                "Microsoft.PowerShellPreview_8wekyb3d8bbwe",
            ]
            .map(|package| apps.join(package).join("pwsh.exe"))
            .into_iter()
            .filter(exists),
        );
    }
    for path in pwsh {
        let preview = text(&path).to_ascii_lowercase().contains("preview");
        shells.push(Shell::new(
            if preview {
                "PowerShell Preview"
            } else {
                "PowerShell"
            },
            text(&path),
            &[],
        ));
    }

    let wsl = text(&system.join("wsl.exe"));
    for distribution in wsl_distributions() {
        shells.push(Shell {
            args: vec!["-d".into(), distribution.clone()],
            ..Shell::new(distribution, &wsl, &[])
        });
    }

    for folder in [
        env("ProgramFiles"),
        local.as_ref().map(|local| local.join("Programs")),
    ]
    .into_iter()
    .flatten()
    {
        let bash = folder.join(r"Git\bin\bash.exe");
        if exists(&bash) {
            shells.push(Shell::new("Git Bash", text(&bash), &["-i", "-l"]));
        }
    }

    shells.extend(visual_studio(&cmd, &text(&powershell)));
    if let Some(local) = &local {
        shells.extend(windows_terminal(local, env("ProgramData").as_deref()));
    }
    let default = std::env::var("ComSpec").unwrap_or(cmd);
    finish(shells, Some(&default))
}

/// Registered WSL distributions, without Docker Desktop's internal ones.
#[cfg(windows)]
fn wsl_distributions() -> Vec<String> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let Ok(lxss) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Lxss")
    else {
        return Vec::new();
    };
    lxss.enum_keys()
        .flatten()
        .filter_map(|key| {
            lxss.open_subkey(key)
                .ok()?
                .get_value::<String, _>("DistributionName")
                .ok()
        })
        .filter(|name| !name.is_empty() && !name.starts_with("docker-desktop"))
        .collect()
}

/// Developer prompts for each Visual Studio installation, as Windows Terminal
/// creates them. `-startdir=none` and `-SkipAutomaticLocation` keep the
/// terminal's folder.
#[cfg(windows)]
fn visual_studio(cmd: &str, powershell: &str) -> Vec<Shell> {
    use std::os::windows::process::CommandExt;
    let Some(programs) = std::env::var_os("ProgramFiles(x86)") else {
        return Vec::new();
    };
    let vswhere =
        std::path::Path::new(&programs).join(r"Microsoft Visual Studio\Installer\vswhere.exe");
    if !vswhere.is_file() {
        return Vec::new();
    }
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let Ok(output) = std::process::Command::new(vswhere)
        .args(["-all", "-prerelease", "-format", "json", "-utf8"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86" => "x86",
        _ => "x64",
    };
    let instances: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).unwrap_or_default();
    let mut shells = Vec::new();
    for instance in instances {
        let (Some(id), Some(path)) = (
            instance["instanceId"].as_str(),
            instance["installationPath"].as_str(),
        ) else {
            continue;
        };
        let version = instance["catalog"]["productLineVersion"]
            .as_str()
            .unwrap_or_default();
        let tools = std::path::Path::new(path).join(r"Common7\Tools");
        let dev_cmd = tools.join("VsDevCmd.bat");
        if dev_cmd.is_file() {
            shells.push(Shell {
                args: vec![
                    "/k".into(),
                    dev_cmd.to_string_lossy().into_owned(),
                    "-startdir=none".into(),
                    format!("-arch={arch}"),
                    format!("-host_arch={arch}"),
                ],
                ..Shell::new(
                    format!("Developer Command Prompt for VS {version}"),
                    cmd,
                    &[],
                )
            });
        }
        let dev_shell = tools.join("Microsoft.VisualStudio.DevShell.dll");
        if dev_shell.is_file() {
            // Single-quoted PowerShell strings, so the argument has no `"`
            // for the Windows command line to escape.
            let module = dev_shell.to_string_lossy().replace('\'', "''");
            shells.push(Shell {
                args: vec![
                    "-NoExit".into(),
                    "-Command".into(),
                    format!(
                        "&{{Import-Module '{module}'; Enter-VsDevShell {id} -SkipAutomaticLocation -DevCmdArguments '-arch={arch} -host_arch={arch}'}}"
                    ),
                ],
                ..Shell::new(format!("Developer PowerShell for VS {version}"), powershell, &[])
            });
        }
    }
    shells
}

/// Profiles with their own command line in Windows Terminal's settings and
/// fragments, such as the Anaconda prompts. Profiles Windows Terminal
/// generates store no command line; [`detect`] finds those itself.
#[cfg(windows)]
fn windows_terminal(local: &std::path::Path, program_data: Option<&std::path::Path>) -> Vec<Shell> {
    let packages = local.join("Packages");
    let mut files: Vec<std::path::PathBuf> = [
        "Microsoft.WindowsTerminal_8wekyb3d8bbwe",
        "Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe",
    ]
    .map(|package| packages.join(package).join(r"LocalState\settings.json"))
    .into();
    files.push(local.join(r"Microsoft\Windows Terminal\settings.json"));
    let settings = files.len();
    for root in [Some(local), program_data].into_iter().flatten() {
        let fragments = root.join(r"Microsoft\Windows Terminal\Fragments");
        for app in std::fs::read_dir(fragments).into_iter().flatten().flatten() {
            for file in std::fs::read_dir(app.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                if file
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
                {
                    files.push(file.path());
                }
            }
        }
    }
    let mut hidden = Vec::new();
    let mut shells = Vec::new();
    for (index, file) in files.iter().enumerate() {
        // Bounded: settings files are small, and a damaged one is skipped.
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        if text.len() > 4 * 1024 * 1024 {
            continue;
        }
        let (found, names) = terminal_profiles(&text);
        if index < settings {
            hidden.extend(names);
        }
        shells.extend(found);
    }
    shells.retain(|shell| !hidden.contains(&shell.name));
    shells
}

/// Visible profiles with a command line, and the names of hidden profiles.
#[cfg(any(windows, test))]
fn terminal_profiles(text: &str) -> (Vec<Shell>, Vec<String>) {
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&strip_jsonc(text)) else {
        return Default::default();
    };
    let profiles = &settings["profiles"];
    let list = profiles["list"].as_array().or(profiles.as_array());
    let mut shells = Vec::new();
    let mut hidden = Vec::new();
    for profile in list.into_iter().flatten() {
        let Some(name) = profile["name"]
            .as_str()
            .filter(|name| !name.trim().is_empty())
        else {
            continue;
        };
        if profile["hidden"].as_bool() == Some(true) {
            hidden.push(name.to_owned());
            continue;
        }
        let Some(command) = profile["commandline"].as_str() else {
            continue;
        };
        let mut args = split_command_line(&expand_variables(command));
        if args.is_empty() {
            continue;
        }
        let program = args.remove(0);
        shells.push(Shell {
            name: name.to_owned(),
            program,
            args,
            default: false,
        });
    }
    (shells, hidden)
}

/// Replaces `%NAME%` with its environment value; unknown names stay as written.
#[cfg(any(windows, test))]
fn expand_variables(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%').map(|end| (&after[..end], end)) {
            Some((name, end)) if !name.is_empty() => {
                match std::env::var(name) {
                    Ok(value) => out.push_str(&value),
                    Err(_) => out.push_str(&rest[start..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Splits a command line as `CommandLineToArgvW` does: whitespace separates
/// arguments outside quotes, `""` inside quotes is a quote, and backslashes
/// are literal unless they precede a quote.
#[cfg(any(windows, test))]
fn split_command_line(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                if chars.peek() == Some(&'"') {
                    current.extend(std::iter::repeat_n('\\', count / 2));
                    if count % 2 == 1 {
                        chars.next();
                        current.push('"');
                    }
                } else {
                    current.extend(std::iter::repeat_n('\\', count));
                }
                started = true;
            }
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            '"' => {
                quoted = !quoted;
                started = true;
            }
            ' ' | '\t' if !quoted => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(current);
    }
    args
}

/// Windows Terminal's settings allow comments and trailing commas.
#[cfg(any(windows, test))]
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut string = false;
    while let Some(c) = chars.next() {
        if string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                string = true;
                out.push(c);
            }
            ('/', Some('/')) => while chars.next_if(|c| *c != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut previous = ' ';
                for c in chars.by_ref() {
                    if previous == '*' && c == '/' {
                        break;
                    }
                    previous = c;
                }
                out.push(' ');
            }
            (c @ ('}' | ']'), _) => {
                let kept = out.trim_end().len();
                if out[..kept].ends_with(',') {
                    out.truncate(kept - 1);
                }
                out.push(c);
            }
            (c, _) => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_lines_split_like_windows() {
        assert_eq!(
            split_command_line(r#"C:\Windows\System32\cmd.exe "/K" C:\a\activate.bat C:\a"#),
            [
                r"C:\Windows\System32\cmd.exe",
                "/K",
                r"C:\a\activate.bat",
                r"C:\a"
            ]
        );
        assert_eq!(
            split_command_line(
                r#"powershell.exe -NoExit -Command "& 'C:\a b\hook.ps1' ; conda activate 'C:\a b' ""#
            ),
            [
                "powershell.exe",
                "-NoExit",
                "-Command",
                r"& 'C:\a b\hook.ps1' ; conda activate 'C:\a b' "
            ]
        );
        assert_eq!(
            split_command_line(r#""C:\Program Files\x.exe"  a\\\"b "c""d" "" e\\"#),
            [r"C:\Program Files\x.exe", r#"a\"b"#, r#"c"d"#, "", r"e\\"]
        );
        assert!(split_command_line("   ").is_empty());
    }

    #[test]
    fn variables_expand_and_unknown_ones_stay() {
        // SAFETY: a variable no other test reads.
        unsafe { std::env::set_var("NEPTUNE_SHELLS_TEST", r"C:\Windows") };
        assert_eq!(
            expand_variables(r"%NEPTUNE_SHELLS_TEST%\cmd.exe %NEPTUNE_MISSING% 50% %"),
            r"C:\Windows\cmd.exe %NEPTUNE_MISSING% 50% %"
        );
    }

    #[test]
    fn terminal_profiles_skip_generated_and_hidden_entries() {
        let settings = r#"{
            // A comment, and a URL inside a string: "http://x"
            "$help": "https://aka.ms/terminal-documentation",
            "profiles": {
                "defaults": {},
                "list": [
                    { "name": "Command Prompt", "commandline": "cmd.exe", },
                    { "name": "PowerShell", "source": "Windows.Terminal.PowershellCore" },
                    /* block */ { "name": "Old", "commandline": "old.exe", "hidden": true },
                    { "name": "Anaconda", "commandline": "cmd.exe \"/K\" activate.bat" },
                ],
            },
        }"#;
        let (shells, hidden) = terminal_profiles(settings);
        assert_eq!(hidden, ["Old"]);
        assert_eq!(shells.len(), 2);
        assert_eq!(shells[1].name, "Anaconda");
        assert_eq!(shells[1].program, "cmd.exe");
        assert_eq!(shells[1].args, ["/K", "activate.bat"]);
        // Fragments list profiles directly.
        let (fragment, _) =
            terminal_profiles(r#"{"profiles":[{"name":"Tool","commandline":"tool.exe -x"}]}"#);
        assert_eq!(fragment[0].args, ["-x"]);
        assert!(terminal_profiles("not json").0.is_empty());
    }

    #[test]
    fn finished_lists_drop_repeats_mark_the_default_and_name_twins() {
        let shells = finish(
            vec![
                Shell::new("PowerShell", "/a/pwsh", &[]),
                Shell::new("zsh", "/bin/zsh", &[]),
                Shell::new("PowerShell", "/b/pwsh", &[]),
                Shell::new("zsh again", "/bin/zsh", &[]),
                Shell::new("Ubuntu", "wsl", &["-d", "Ubuntu"]),
            ],
            Some("/bin/zsh"),
        );
        let names: Vec<_> = shells.iter().map(|shell| shell.name.as_str()).collect();
        assert_eq!(
            names,
            ["PowerShell (/a)", "zsh", "PowerShell (/b)", "Ubuntu"]
        );
        assert!(shells[1].default);
        assert!(shells.iter().filter(|shell| shell.default).count() == 1);
        assert!(shells[3].runs("wsl", &["-d".into(), "Ubuntu".into()]));
        assert!(!shells[3].runs("wsl", &[]));
    }
}
