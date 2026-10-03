//! The tools Neptune offers a CLI agent, served over the agent's own stdio
//! pipes. Only a pull request address leaves this process.
use neptune_model::PullRequest;
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};

const MAX_LINE: u64 = 64 * 1024;
const TOOL: &str = "link_pull_request";
const INSTRUCTIONS: &str = "This terminal runs in Neptune, which shows the pull requests you link next to the terminal's tab. \
Call link_pull_request with the pull request's URL right after you create a pull request, and when you start work on an existing one. \
Link every pull request of a stack. Linking the same pull request again is safe.";

/// Answers one JSON-RPC message; notifications have no answer. `link` hands an
/// address to Neptune and reports whether this terminal took it.
pub fn respond(message: &Value, link: &mut dyn FnMut(&str) -> bool) -> Option<Value> {
    let id = message.get("id").filter(|id| !id.is_null())?.clone();
    let result = match message.get("method").and_then(Value::as_str) {
        Some("initialize") => json!({
            "protocolVersion": message
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "neptune", "version": env!("CARGO_PKG_VERSION")},
            "instructions": INSTRUCTIONS,
        }),
        Some("ping") => json!({}),
        Some("tools/list") => json!({"tools": [{
            "name": TOOL,
            "description": "Link a pull request you created or are working on to this Neptune terminal tab, where its number opens it in the browser.",
            "inputSchema": {
                "type": "object",
                "properties": {"url": {
                    "type": "string",
                    "description": "The pull request's address, such as https://github.com/owner/repo/pull/123",
                }},
                "required": ["url"],
                "additionalProperties": false,
            },
        }]}),
        Some("tools/call") if message.pointer("/params/name") == Some(&json!(TOOL)) => {
            let url = message
                .pointer("/params/arguments/url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let (text, failed) = match PullRequest::parse(url) {
                None => (
                    "Not linked: expected a pull request address such as https://github.com/owner/repo/pull/123".to_owned(),
                    true,
                ),
                Some(pull_request) if link(pull_request.url()) => (
                    format!("Linked {} to this terminal tab.", pull_request.label()),
                    false,
                ),
                Some(_) => (
                    "Not linked: Neptune is not tracking an agent in this terminal.".to_owned(),
                    true,
                ),
            };
            json!({"content": [{"type": "text", "text": text}], "isError": failed})
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": "Method not found"},
            }));
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Serves newline-delimited messages until the agent closes its end.
pub fn serve(
    input: impl Read,
    mut output: impl Write,
    link: &mut dyn FnMut(&str) -> bool,
) -> std::io::Result<()> {
    let mut input = std::io::BufReader::new(input);
    let mut line = Vec::new();
    loop {
        line.clear();
        if (&mut input).take(MAX_LINE).read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        if line.last() != Some(&b'\n') && line.len() as u64 == MAX_LINE {
            // An oversized message is dropped whole, not read as several.
            let mut rest = Vec::new();
            while (&mut input).take(MAX_LINE).read_until(b'\n', &mut rest)? > 0
                && rest.last() != Some(&b'\n')
            {
                rest.clear();
            }
            continue;
        }
        if let Ok(message) = serde_json::from_slice::<Value>(&line)
            && let Some(answer) = respond(&message, link)
        {
            serde_json::to_writer(&mut output, &answer)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_server_introduces_its_tool_and_links_only_pull_request_addresses() {
        let mut linked = Vec::new();
        let mut output = Vec::new();
        let call = |id: u64, url: &str| {
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":TOOL,"arguments":{"url":url}}})
                .to_string()
        };
        let oversized = format!(
            "{{\"id\":9,\"method\":\"ping\",\"pad\":\"{}\"}}",
            "x".repeat(200_000)
        );
        let input = [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}).to_string(),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}).to_string(),
            call(3, "https://github.com/zevem/neptune/pull/83/files"),
            call(4, "https://example.com/not/a/pr"),
            oversized,
            "not json".into(),
            call(5, "https://github.com/zevem/neptune/pull/7"),
            json!({"jsonrpc":"2.0","id":6,"method":"resources/list"}).to_string(),
        ]
        .join("\n");
        serve(input.as_bytes(), &mut output, &mut |url| {
            linked.push(url.to_owned());
            linked.len() == 1
        })
        .unwrap();
        let answers: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let ids: Vec<_> = answers
            .iter()
            .map(|answer| answer["id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids, [1, 2, 3, 4, 5, 6]);
        assert_eq!(answers[0]["result"]["protocolVersion"], "2024-11-05");
        assert!(
            answers[0]["result"]["instructions"]
                .as_str()
                .unwrap()
                .contains(TOOL)
        );
        assert_eq!(answers[1]["result"]["tools"][0]["name"], TOOL);
        assert_eq!(answers[2]["result"]["isError"], false);
        assert_eq!(
            answers[2]["result"]["content"][0]["text"],
            "Linked zevem/neptune#83 to this terminal tab."
        );
        assert_eq!(answers[3]["result"]["isError"], true);
        assert_eq!(answers[4]["result"]["isError"], true);
        assert_eq!(answers[5]["error"]["code"], -32601);
        assert_eq!(
            linked,
            [
                "https://github.com/zevem/neptune/pull/83",
                "https://github.com/zevem/neptune/pull/7"
            ]
        );
    }
}
