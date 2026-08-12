//! The agent surface: the same screen over the Model Context Protocol
//! on stdio, so a model can ask what is hiding in a document rather than
//! be handed the document and try to see it — which, for characters that
//! render as nothing, it cannot.
//!
//! Two rules the family's MCP surfaces established:
//!
//! - **An empty answer is not an error.** A clean document comes back as
//!   an ordinary result carrying `ok: true` — the scan ran. Only a
//!   malformed question is a protocol error.
//! - **Refusals speak the caller's vocabulary.** An MCP caller has no
//!   command line, so no message here mentions a flag.
//!
//! Read-only by construction: nothing on this surface writes.

pub(crate) mod detect;

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::{Value, json};

use crate::scan;
use crate::walk::{self, WalkOptions};

const PROTOCOL_VERSION: &str = "2025-06-18";

/// JSON-RPC error codes, from the spec.
const INVALID_PARAMS: i64 = -32602;
const METHOD_NOT_FOUND: i64 = -32601;

pub(crate) fn serve() -> ExitCode {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            return ExitCode::from(2);
        };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            // A frame that is not JSON has no id to answer against;
            // dropping it is the only honest option.
            continue;
        };
        let Some(response) = handle(&request) else {
            continue; // a notification: no reply
        };
        if writeln!(stdout, "{response}").is_err() || stdout.flush().is_err() {
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method")?.as_str()?;
    // Notifications carry no id and get no reply.
    id.as_ref()?;

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "unicode-le", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(request.get("params")),
        "ping" => Ok(json!({})),
        other => Err((
            METHOD_NOT_FOUND,
            format!("this server does not implement {other}"),
        )),
    };

    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }),
    })
}

fn tool_definitions() -> Value {
    json!([
        detect::definition(),
        {
            "name": "unicode_le_scan",
            "description": "Run the same screen over files or directories and return one report \
                            for the run. Reads the filesystem; never writes to it. Findings \
                            carry codepoints as U+XXXX and never the characters themselves.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "a file or directory to scan" },
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "several files or directories, instead of `path`",
                    },
                    "kinds": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Report only these kinds. Omit for all of them.",
                    },
                    "scripts": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Non-Latin scripts this tree is expected to contain, by \
                                        Unicode name or ISO 15924 tag.",
                    },
                    "hidden": {
                        "type": "boolean",
                        "default": false,
                        "description": "Walk hidden files and directories too.",
                    },
                    "ignored": {
                        "type": "boolean",
                        "default": false,
                        "description": "Walk files excluded by .gitignore too.",
                    },
                },
            },
        },
    ])
}

/// Protocol failures (no tool named, an unknown tool) are JSON-RPC
/// errors; a tool that fails on its arguments returns a result carrying
/// `isError`, so a model reads the reason and reacts rather than
/// concluding the server is broken.
fn call_tool(params: Option<&Value>) -> Result<Value, (i64, String)> {
    let params = params.ok_or((INVALID_PARAMS, "no tool call was supplied".to_string()))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or((INVALID_PARAMS, "the tool call named no tool".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        "detect_unicode_risks" => Ok(match detect::run(&arguments) {
            Ok(result) => tool_result(&result),
            Err(message) => tool_failure(&message),
        }),
        "unicode_le_scan" => Ok(match scan_tool(&arguments) {
            Ok(result) => tool_result(&result),
            Err(message) => tool_failure(&message),
        }),
        other => Err((
            INVALID_PARAMS,
            format!("this server offers no tool named {other}"),
        )),
    }
}

fn scan_tool(arguments: &Value) -> Result<Value, String> {
    let inputs = requested_paths(arguments)?;
    let options = detect::read_options(arguments)?;
    let flag = |name: &str| {
        arguments
            .get(name)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let walk_options = WalkOptions {
        hidden: flag("hidden"),
        respect_ignore: !flag("ignored"),
    };

    let walked = walk::collect(&inputs, &walk_options)?;
    let report = scan::report(
        walked
            .files
            .iter()
            .map(|target| scan::scan_file(target, &options))
            .collect(),
    );

    // One diagnostic per refused file, so a model reading only the text
    // cannot take an empty finding list for a clean tree.
    let diagnostics: Vec<Value> = report
        .files
        .iter()
        .flat_map(|file| {
            file.refusals
                .iter()
                .map(|refusal| warning(&file.file, &refusal.detail))
        })
        .collect();
    let count = report.summary.files;
    let data = json!({ "report": serde_json::to_value(&report).expect("a report serializes") });
    Ok(envelope(
        "unicode_le_scan",
        &data,
        count,
        &diagnostics,
        false,
    ))
}

fn requested_paths(arguments: &Value) -> Result<Vec<PathBuf>, String> {
    if let Some(path) = arguments.get("path").and_then(Value::as_str) {
        return Ok(vec![PathBuf::from(path)]);
    }
    if let Some(items) = arguments.get("paths").and_then(Value::as_array) {
        let paths: Vec<PathBuf> = items
            .iter()
            .filter_map(|item| item.as_str().map(PathBuf::from))
            .collect();
        if paths.is_empty() {
            return Err("the list of paths was empty".to_string());
        }
        return Ok(paths);
    }
    Err("no file or directory was supplied to scan".to_string())
}

/// The one result shape every tool returns: `{ ok, data, diagnostics,
/// meta }`.
///
/// **`ok` reports whether the check ran, not whether the answer is
/// yes.** A file full of bidi controls is the answer, not a failure to
/// produce one — conflating the two would have a model report a broken
/// tool when what it actually learned is that the file is dangerous.
pub(crate) fn envelope(
    tool: &str,
    data: &Value,
    count: usize,
    diagnostics: &[Value],
    truncated: bool,
) -> Value {
    let ok = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic["severity"].as_str() == Some("error"));
    json!({
        "ok": ok,
        "data": data,
        "diagnostics": diagnostics,
        "meta": { "tool": tool, "count": count, "truncated": truncated },
    })
}

/// An MCP tool result: the envelope as text (what a model reads) and the
/// same envelope structured.
fn tool_result(envelope: &Value) -> Value {
    let text = serde_json::to_string_pretty(envelope).expect("an envelope serializes");
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": envelope,
        "isError": false,
    })
}

fn warning(file: &str, message: &str) -> Value {
    json!({ "severity": "warning", "code": "refused", "message": format!("{file}: {message}") })
}

/// The tool could not run on the arguments given. `isError` so a model
/// reads the message and corrects itself.
fn tool_failure(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempTree;

    fn request(method: &str, params: &Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    fn call(name: &str, arguments: &Value) -> Value {
        handle(&request(
            "tools/call",
            &json!({ "name": name, "arguments": arguments }),
        ))
        .expect("a reply")
    }

    #[test]
    fn initialize_answers_with_the_protocol_version() {
        let response = handle(&request("initialize", &json!({}))).expect("a reply");
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], "unicode-le");
    }

    #[test]
    fn tools_list_offers_both_tools() {
        let response = handle(&request("tools/list", &json!({}))).expect("a reply");
        let tools = response["result"]["tools"].as_array().expect("tools");
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(names, ["detect_unicode_risks", "unicode_le_scan"]);
    }

    #[test]
    fn a_notification_gets_no_reply() {
        let notification = json!({ "jsonrpc": "2.0", "method": "initialized" });
        assert!(handle(&notification).is_none());
    }

    #[test]
    fn an_unknown_method_is_a_protocol_error() {
        let response = handle(&request("does/not/exist", &json!({}))).expect("a reply");
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn an_unknown_tool_is_a_protocol_error() {
        assert_eq!(
            call("unicode_le_fix", &json!({}))["error"]["code"],
            INVALID_PARAMS
        );
    }

    /// A bad argument is the tool failing on what it was given, not the
    /// server breaking — so it comes back as a result carrying isError.
    #[test]
    fn a_missing_argument_is_a_tool_failure_not_a_protocol_error() {
        let response = call("unicode_le_scan", &json!({}));
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("a message")
                .contains("no file or directory")
        );
    }

    #[test]
    fn the_document_tool_reports_what_it_found() {
        let response = call(
            "detect_unicode_risks",
            &json!({ "content": "let a = \"\u{202E}\";" }),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(envelope["meta"]["tool"], "detect_unicode_risks");
        assert_eq!(envelope["data"]["findings"][0]["kind"], "bidi-control");
        assert_eq!(envelope["data"]["findings"][0]["codepoints"][0], "U+202E");
        assert_eq!(envelope["ok"], true);
        assert_eq!(response["result"]["isError"], false);
    }

    /// An empty answer is the scan running and finding nothing.
    #[test]
    fn a_clean_document_is_an_ordinary_result() {
        let response = call(
            "detect_unicode_risks",
            &json!({ "content": "const x = 1;" }),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["meta"]["count"], 0);
    }

    /// **The property this surface exists for.** A model that is handed
    /// the answer must not be handed the attack with it.
    #[test]
    fn nothing_the_server_returns_carries_a_dangerous_character() {
        let content = "p\u{430}ypal \u{202E}txt.exe x\u{200B}y \u{FF26}";
        let response = call("detect_unicode_risks", &json!({ "content": content }));
        let findings = serde_json::to_string(&response["result"]["structuredContent"]["data"])
            .expect("serializes");
        assert!(findings.is_ascii(), "{findings}");
    }

    #[test]
    fn a_refusal_comes_back_structured_and_as_a_diagnostic() {
        let response = call(
            "detect_unicode_risks",
            &json!({ "content": "ключ: значение\nдругой: текст" }),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(
            envelope["data"]["refusals"][0]["reason"],
            "intentional_script_context"
        );
        assert_eq!(envelope["diagnostics"][0]["severity"], "warning");
        // A refusal is not the tool breaking: the scan ran, and every
        // other check ran on that file.
        assert_eq!(envelope["ok"], true);
    }

    /// Naming the script is what lets the check run on a translated
    /// document, and it must actually work on this surface too.
    #[test]
    fn naming_the_script_lifts_the_refusal() {
        let response = call(
            "detect_unicode_risks",
            &json!({ "content": "ключ: значение\nдругой: текст", "scripts": ["Cyrillic"] }),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(
            envelope["data"]["refusals"]
                .as_array()
                .expect("a list")
                .len(),
            0
        );
        assert_eq!(envelope["diagnostics"].as_array().expect("a list").len(), 0);
    }

    #[test]
    fn the_scan_tool_reports_what_it_found() {
        let tree = TempTree::new("mcp-scan");
        tree.write("src/a.ts", "const a = \"\u{202E}\";\n");
        let response = call(
            "unicode_le_scan",
            &json!({ "path": tree.path().to_string_lossy() }),
        );
        let envelope = &response["result"]["structuredContent"];
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(envelope["data"]["report"]["schema"], 1);
        assert_eq!(envelope["data"]["report"]["summary"]["findings"], 1);
        assert_eq!(envelope["data"]["report"]["summary"]["bidi"], 1);
    }

    /// Refusals speak the caller's vocabulary: an MCP caller has no
    /// command line, so no message may name a flag.
    #[test]
    fn no_message_mentions_a_command_line_flag() {
        let definitions = serde_json::to_string(&tool_definitions()).expect("serializes");
        assert!(!definitions.contains("--"), "{definitions}");

        let tree = TempTree::new("mcp-vocabulary");
        tree.write("a.ts", "const a = 1;\n");
        for (tool, arguments) in [
            ("unicode_le_scan", json!({})),
            ("unicode_le_scan", json!({ "paths": [] })),
            ("unicode_le_scan", json!({ "path": "/no/such/place-xyz" })),
            (
                "unicode_le_scan",
                json!({ "path": tree.path().to_string_lossy() }),
            ),
            (
                "detect_unicode_risks",
                json!({ "content": "你好世界你好世界" }),
            ),
            (
                "detect_unicode_risks",
                json!({ "content": "x", "kinds": ["nope"] }),
            ),
        ] {
            let rendered = serde_json::to_string(&call(tool, &arguments)).expect("serializes");
            assert!(!rendered.contains("--"), "{tool}: {rendered}");
        }
    }

    /// Every tool returns the same envelope, so a caller writes one
    /// reader for all of them.
    #[test]
    fn every_tool_returns_the_same_envelope_shape() {
        let tree = TempTree::new("mcp-envelope");
        tree.write("a.md", "x");
        let results = [
            call("detect_unicode_risks", &json!({ "content": "x" })),
            call(
                "unicode_le_scan",
                &json!({ "path": tree.path().to_string_lossy() }),
            ),
        ];
        for result in results {
            let envelope = &result["result"]["structuredContent"];
            assert!(envelope["ok"].is_boolean(), "{envelope}");
            assert!(!envelope["data"].is_null(), "{envelope}");
            assert!(envelope["diagnostics"].is_array(), "{envelope}");
            assert!(envelope["meta"]["tool"].is_string(), "{envelope}");
            assert!(envelope["meta"]["count"].is_number(), "{envelope}");
            assert!(envelope["meta"]["truncated"].is_boolean(), "{envelope}");
        }
    }
}
