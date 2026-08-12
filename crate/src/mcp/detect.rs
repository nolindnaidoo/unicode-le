//! `detect_unicode_risks` — the tool an agent reaches for.
//!
//! It touches no filesystem. An agent already has file-read tools;
//! duplicating them here would add a path-traversal surface for no
//! capability. The tool that needs a filesystem is `unicode_le_scan`.
//!
//! This is also the tool that matters most to a model, for a reason
//! specific to this crate: a model that pastes a document into its own
//! reasoning has already been handed the bidi controls in it. What comes
//! back from here is `U+XXXX` and English, so the *answer* cannot carry
//! the attack onward into a commit message, a review comment or a reply.

use serde_json::{Value, json};

use crate::detect::{self, KINDS, Options};

const DEFAULT_MAX_RESULTS: usize = 500;
const MAX_MAX_RESULTS: usize = 5000;

pub(crate) fn definition() -> Value {
    let kinds: Vec<&str> = KINDS.iter().map(|(_, _, short)| *short).collect();
    json!({
        "name": "detect_unicode_risks",
        "description": "Find the Unicode characters in a document that hide meaning: \
                        bidirectional controls (the Trojan Source class, CVE-2021-42574), \
                        invisible characters, homoglyphs, words that mix scripts, text that is \
                        not in NFC, spaces that are not the space, and unassigned or private-use \
                        codepoints. Each finding carries a 1-based line and column, a byte \
                        offset, the codepoints as U+XXXX, the script and a severity. It never \
                        returns the offending characters themselves, and never rewrites \
                        anything. A document plainly written in a non-Latin script is not judged \
                        for homoglyphs unless that script is named in `scripts`; it says so in \
                        `refusals` rather than guessing.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "The document text to scan." },
                "kinds": {
                    "type": "array",
                    "items": { "type": "string", "enum": kinds },
                    "description": "Report only these kinds. Omit for all of them, which is \
                                    the only setting under which an empty result means clean.",
                },
                "scripts": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Non-Latin scripts this document is expected to be written \
                                    in, by Unicode name or ISO 15924 tag, e.g. Han or Cyrillic. \
                                    Naming them is what allows the homoglyph check to run on a \
                                    translated document.",
                },
                "maxResults": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_MAX_RESULTS,
                    "default": DEFAULT_MAX_RESULTS,
                    "description": format!(
                        "Cap on returned findings (default {DEFAULT_MAX_RESULTS}). \
                         meta.truncated reports whether any were dropped."
                    ),
                },
            },
            "required": ["content"],
            "additionalProperties": false,
        },
    })
}

pub(crate) fn run(arguments: &Value) -> Result<Value, String> {
    let content = arguments
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "content is required and must be a string".to_string())?;
    let max_results = read_max_results(arguments)?;
    let options = read_options(arguments)?;

    let examination = detect::examine(content, &options);
    let mut findings: Vec<Value> = examination
        .findings
        .iter()
        .map(|finding| serde_json::to_value(finding).expect("a finding serializes"))
        .collect();

    // The `truncated` flag matters more than the cap: a silently
    // incomplete answer is wrong in the most expensive way.
    let truncated = findings.len() > max_results;
    findings.truncate(max_results);

    // A refusal is carried twice on purpose: structured, so a caller can
    // branch on it, and as a diagnostic, so a model reading the text
    // cannot miss that part of the document was not judged.
    let refusals: Vec<Value> = examination
        .refusals
        .iter()
        .map(|refusal| serde_json::to_value(refusal).expect("a refusal serializes"))
        .collect();
    let diagnostics: Vec<Value> = examination
        .refusals
        .iter()
        .map(|refusal| {
            json!({
                "severity": "warning",
                "code": serde_json::to_value(refusal.reason).expect("a reason serializes"),
                "message": refusal.detail,
            })
        })
        .collect();

    let count = findings.len();
    Ok(super::envelope(
        "detect_unicode_risks",
        &json!({ "findings": findings, "refusals": refusals }),
        count,
        &diagnostics,
        truncated,
    ))
}

pub(crate) fn read_options(arguments: &Value) -> Result<Options, String> {
    let mut options = Options::default();
    for token in strings(arguments, "kinds")? {
        options.kinds.push(detect::parse_kind(&token)?);
    }
    for token in strings(arguments, "scripts")? {
        options
            .expected_scripts
            .push(detect::scripts::parse_script(&token)?);
    }
    Ok(options)
}

fn strings(arguments: &Value, name: &str) -> Result<Vec<String>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("{name} must be a list of strings"))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{name} must be a list of strings"))
        })
        .collect()
}

/// Clamp quietly, reject loudly — the family's asymmetry.
fn read_max_results(arguments: &Value) -> Result<usize, String> {
    let Some(raw) = arguments.get("maxResults") else {
        return Ok(DEFAULT_MAX_RESULTS);
    };
    let invalid = "maxResults must be a positive integer".to_string();
    let value = raw.as_u64().ok_or(invalid.clone())?;
    if value < 1 {
        return Err(invalid);
    }
    Ok(usize::try_from(value)
        .unwrap_or(MAX_MAX_RESULTS)
        .min(MAX_MAX_RESULTS))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;
    use crate::detect::corpus::document;

    const CASES: &str = include_str!("../../fixtures/mcp-detect-unicode.json");

    #[derive(Debug, Deserialize)]
    struct Case {
        name: String,
        file: Option<String>,
        content: Option<String>,
        arguments: Value,
        expected: Option<Value>,
        #[serde(rename = "expectedError")]
        expected_error: Option<String>,
    }

    #[test]
    fn every_corpus_case_answers_as_pinned() {
        let cases: Vec<Case> = serde_json::from_str(CASES).expect("the corpus is valid JSON");
        assert!(!cases.is_empty(), "the corpus is empty");

        for case in cases {
            let mut arguments = case.arguments.clone();
            let content = case
                .file
                .as_deref()
                .map(document)
                .map(str::to_string)
                .or(case.content);
            if let Some(content) = content {
                arguments["content"] = json!(content);
            }

            match (case.expected, case.expected_error) {
                (_, Some(expected)) => {
                    assert_eq!(
                        run(&arguments).expect_err(&case.name),
                        expected,
                        "{}",
                        case.name
                    );
                }
                (Some(expected), None) => {
                    assert_eq!(
                        run(&arguments).expect(&case.name),
                        expected,
                        "{}",
                        case.name
                    );
                }
                (None, None) => panic!("{} pins neither a result nor an error", case.name),
            }
        }
    }

    #[test]
    fn the_tool_name_is_pinned() {
        assert_eq!(definition()["name"], "detect_unicode_risks");
    }

    /// The schema may not offer to change anything, because nothing
    /// here does.
    #[test]
    fn the_schema_offers_no_way_to_rewrite_a_document() {
        let definition = definition();
        let properties = definition["inputSchema"]["properties"]
            .as_object()
            .expect("properties");
        for absent in ["fix", "normalize", "form", "output", "replace"] {
            assert!(!properties.contains_key(absent), "{absent} is offered");
        }
    }

    #[test]
    fn a_fractional_cap_is_refused() {
        let error = run(&json!({ "content": "x", "maxResults": 1.5 })).expect_err("a refusal");
        assert_eq!(error, "maxResults must be a positive integer");
    }

    #[test]
    fn a_bad_kind_or_script_is_refused_by_name() {
        assert!(
            run(&json!({ "content": "x", "kinds": ["homoglyph"] }))
                .expect_err("a refusal")
                .contains("homoglyph")
        );
        assert!(
            run(&json!({ "content": "x", "scripts": ["Klingon"] }))
                .expect_err("a refusal")
                .contains("Klingon")
        );
        assert!(
            run(&json!({ "content": "x", "kinds": "bidi" }))
                .expect_err("a refusal")
                .contains("list of strings")
        );
    }
}
