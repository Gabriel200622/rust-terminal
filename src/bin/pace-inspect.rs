//! Developer-only client for an explicitly enabled native egui inspection server.
use std::{env, fs, net::TcpStream, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use eframe::egui::{Event, Key, Modifiers, PointerButton, pos2};
use egui_inspection::{Request, Response, protocol};

const HELP: &str = "pace-inspect [--addr HOST:PORT] COMMAND\n\
\n\
  info                         Print application/protocol information\n\
  tree                         Print the current AccessKit tree as JSON\n\
  screenshot PATH              Save a native screenshot at 1 px/logical point\n\
  key KEY [--ctrl --shift --alt --cmd]\n\
                               Press/release a key or chord (ctrl+shift+t)\n\
  text TEXT                    Inject text, including Unicode\n\
  move X Y                     Move pointer using logical coordinates\n\
  click X Y                    Click using logical window coordinates\n\
  resize WIDTH HEIGHT          Resize using logical dimensions\n\
  settle [MAX_STEPS]            Wait for an idle frame (default: 60)\n\
\n\
Enable the app with EGUI_INSPECTION=1 and --features inspection.\n\
Default address: 127.0.0.1:5719. This client never launches the app.";

struct Client {
    stream: TcpStream,
}

impl Client {
    fn connect(address: &str) -> Result<Self> {
        let mut stream = TcpStream::connect(address)
            .with_context(|| format!("connect to inspection server {address}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(20)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        stream.set_nodelay(true)?;
        let version = protocol::read_handshake(&mut stream).context("inspection handshake")?;
        ensure!(
            version == protocol::PROTOCOL_VERSION,
            "incompatible inspection protocol {version}; expected {}",
            protocol::PROTOCOL_VERSION
        );
        Ok(Self { stream })
    }

    fn request(&mut self, request: Request) -> Result<Response> {
        protocol::write_message(&mut self.stream, &request).context("send inspection request")?;
        let response: Response =
            protocol::read_message(&mut self.stream).context("read inspection response")?;
        if let Response::Error { message } = &response {
            bail!("inspection server: {message}");
        }
        Ok(response)
    }

    fn events(&mut self, events: Vec<Event>) -> Result<()> {
        let response = self.request(Request::ApplyEvents { events })?;
        ensure!(
            matches!(response, Response::Done),
            "unexpected response: {response:?}"
        );
        Ok(())
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_key(arguments: &[String]) -> Result<(Key, Modifiers)> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for argument in arguments {
        for token in argument.split('+') {
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "--ctrl" => modifiers.ctrl = true,
                "shift" | "--shift" => modifiers.shift = true,
                "alt" | "option" | "--alt" => modifiers.alt = true,
                "cmd" | "command" | "meta" | "super" | "--cmd" => {
                    modifiers.mac_cmd = true;
                }
                _ => {
                    ensure!(key.is_none(), "provide exactly one key");
                    // egui accepts canonical names plus conventional single-letter keys.
                    let candidate = Key::from_name(token).or_else(|| {
                        Key::ALL
                            .iter()
                            .copied()
                            .find(|candidate| candidate.name().eq_ignore_ascii_case(token))
                    });
                    key = Some(candidate.with_context(|| format!("unknown key {token:?}"))?);
                }
            }
        }
    }
    modifiers.command = modifiers.ctrl || modifiers.mac_cmd;
    Ok((key.context("provide a key or chord")?, modifiers))
}

fn number<T: std::str::FromStr>(argument: &str, name: &str) -> Result<T> {
    argument
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid {name}: {argument:?}"))
}

fn main() -> Result<()> {
    let mut arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.is_empty()
        || arguments
            .first()
            .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!("{HELP}");
        return Ok(());
    }
    let address = if arguments.first().is_some_and(|arg| arg == "--addr") {
        ensure!(
            arguments.len() >= 3,
            "--addr requires HOST:PORT and a command"
        );
        arguments.remove(0);
        arguments.remove(0)
    } else {
        egui_inspection::DEFAULT_INSPECTION_ADDR.to_owned()
    };
    let command = arguments.remove(0);
    let mut client = Client::connect(&address)?;
    match command.as_str() {
        "info" | "tree" => {
            ensure!(arguments.is_empty(), "{command} takes no arguments");
            let request = if command == "tree" {
                Request::GetTree
            } else {
                Request::GetInfo
            };
            print_json(&client.request(request)?)?;
        }
        "screenshot" => {
            ensure!(arguments.len() == 1, "screenshot requires PATH");
            let response = client.request(Request::GetScreenshot {
                pixels_per_point: Some(1.0),
            })?;
            let Response::Screenshot(image) = response else {
                bail!("unexpected screenshot response: {response:?}");
            };
            let path = Path::new(&arguments[0]);
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, &image.bytes).with_context(|| format!("save {}", path.display()))?;
            print_json(&serde_json::json!({"path": path, "size": image.size}))?;
        }
        "key" => {
            let (key, modifiers) = parse_key(&arguments)?;
            client.events(vec![
                Event::ModifiersChanged(modifiers),
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ])?;
            client.events(vec![
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers,
                },
                Event::ModifiersChanged(Modifiers::NONE),
            ])?;
            print_json(&Response::Done)?;
        }
        "text" => {
            ensure!(
                arguments.len() == 1,
                "text requires a single quoted TEXT argument"
            );
            client.events(vec![
                Event::ModifiersChanged(Modifiers::NONE),
                Event::Text(arguments.remove(0)),
            ])?;
            print_json(&Response::Done)?;
        }
        "move" | "click" => {
            ensure!(
                arguments.len() == 2,
                "{command} requires X Y logical coordinates"
            );
            let x: f32 = number(&arguments[0], "X")?;
            let y: f32 = number(&arguments[1], "Y")?;
            ensure!(x.is_finite() && y.is_finite(), "coordinates must be finite");
            let pos = pos2(x, y);
            // egui resolves interaction against the preceding frame's widget
            // geometry. Establish hover first so a remote click uses the target
            // widget even when the native OS pointer was elsewhere.
            client.events(vec![
                Event::ModifiersChanged(Modifiers::NONE),
                Event::PointerMoved(pos),
            ])?;
            if command == "click" {
                client.events(vec![
                    Event::ModifiersChanged(Modifiers::NONE),
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    },
                ])?;
            }
            print_json(&Response::Done)?;
        }
        "resize" => {
            ensure!(arguments.len() == 2, "resize requires WIDTH HEIGHT");
            let width: u32 = number(&arguments[0], "WIDTH")?;
            let height: u32 = number(&arguments[1], "HEIGHT")?;
            ensure!(width > 0 && height > 0, "dimensions must be positive");
            print_json(&client.request(Request::Resize { width, height })?)?;
        }
        "settle" => {
            ensure!(arguments.len() <= 1, "settle takes at most one MAX_STEPS");
            let max_steps = arguments
                .first()
                .map(|arg| number(arg, "MAX_STEPS"))
                .transpose()?
                .unwrap_or(60);
            ensure!(max_steps > 0, "MAX_STEPS must be positive");
            print_json(&client.request(Request::Settle { max_steps })?)?;
        }
        _ => bail!("unknown command {command:?}\n{HELP}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_include_command_semantics() {
        let (key, modifiers) = parse_key(&["ctrl+shift+t".into()]).unwrap();
        assert_eq!(key, Key::T);
        assert!(modifiers.ctrl && modifiers.shift && modifiers.command);
        assert!(!modifiers.alt && !modifiers.mac_cmd);
        let (key, modifiers) = parse_key(&["Comma".into(), "--cmd".into()]).unwrap();
        assert_eq!(key, Key::Comma);
        assert!(modifiers.mac_cmd && modifiers.command);
    }

    #[test]
    fn malformed_chords_are_rejected() {
        assert!(parse_key(&["ctrl+shift".into()]).is_err());
        assert!(parse_key(&["ctrl+a+b".into()]).is_err());
        assert!(parse_key(&["unknown-key".into()]).is_err());
    }
}
