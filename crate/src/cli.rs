//! The terminal surface.
//!
//! stdout is always protocol — one JSON report for the run. stderr is
//! always for the human, and is a projection of that same report rather
//! than parallel prose.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::detect::{self, Options};
use crate::escape;
use crate::scan::{self, FailOn, FileReport, Report};
use crate::walk::{self, WalkOptions};

const USAGE: &str = "usage: unicode-le [options] <file|dir>...
       unicode-le [options] --stdin
       unicode-le mcp
       unicode-le --version | --help

Finds the Unicode characters in a tree that hide meaning: the Trojan
Source bidirectional controls, invisibles, homoglyphs, words that mix
scripts, text that is not in NFC, spaces that are not the space, and
codepoints with no assigned meaning.

It reports codepoints, never the characters themselves: a report that
quoted them would carry the attack to whoever read it. It never rewrites
a file: what form your text is in is reported, not corrected.

A file plainly written in another script is not judged for confusables,
and says so. Without being told which script to expect there is no way
to tell a forged name from a translated one, and guessing would bury the
real findings under every word of every translation.

Options:
  --kind <kind>     report only these kinds (repeatable, comma-separated):
                    bidi, invisible, confusable, mixed-script, non-nfc,
                    whitespace, unassigned
  --script <tag>    a non-Latin script this tree is expected to contain,
                    by Unicode name or ISO 15924 tag (repeatable,
                    comma-separated), e.g. Han or Cyrillic
  --fail-on <what>  what exits 1: any finding, or bidi for the Trojan
                    Source class alone (default any)
  --strict          exit 2 if any file was refused, rather than reporting
                    it and carrying on
  --stdin           read one document from stdin
  --hidden          walk hidden files and directories too
  --no-ignore       walk files that .gitignore excludes

Exit codes: 0 clean, 1 findings, 2 the question was malformed.";

/// Every flag the parser accepts. Held equal to the flags named in USAGE
/// by a test, and consulted at runtime so the list is what the parser
/// actually honours.
///
/// USAGE itself is held to ASCII by a test as well. `--help` writes it to
/// stdout with nothing between, so an em dash typed into it is a non-ASCII
/// byte on the protocol stream — the one thing this tool promises never to
/// emit, reached through the one string nothing escapes.
const FLAGS: [&str; 7] = [
    "--kind",
    "--script",
    "--fail-on",
    "--strict",
    "--stdin",
    "--hidden",
    "--no-ignore",
];

/// The values `--fail-on` accepts.
const FAIL_ON: [&str; 2] = ["any", "bidi"];

#[derive(Debug)]
struct Arguments {
    strict: bool,
    inputs: Vec<PathBuf>,
    stdin: bool,
    fail_on: FailOn,
    detect: Options,
    walk: WalkOptions,
}

pub(crate) fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Some(first) = args.first() {
        match first.as_str() {
            "mcp" => return crate::mcp::serve(),
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--version" | "-V" => {
                println!("unicode-le {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    match execute(&args) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            // A refusal names the path the caller gave it, and a caller
            // can give it anything — including a name whose own
            // characters reorder the line this prints on.
            eprintln!("unicode-le: {}", escape::text(&message));
            ExitCode::from(2)
        }
    }
}

fn execute(args: &[String]) -> Result<u8, String> {
    let arguments = parse(args)?;
    let files = if arguments.stdin {
        vec![scan_stdin(&arguments.detect)?]
    } else {
        let walked = walk::collect(&arguments.inputs, &arguments.walk)?;
        walked
            .files
            .iter()
            .map(|target| scan::scan_file(target, &arguments.detect))
            .collect()
    };
    let report = scan::report(files);

    let mut stdout = std::io::stdout().lock();
    // Escaped after serialization, never before: `serde_json` would
    // escape the backslash of a pre-escaped path and the report would
    // stop round-tripping. See `escape`.
    let line = escape::json(&serde_json::to_string(&report).expect("a report serializes"));
    writeln!(stdout, "{line}").map_err(|error| format!("could not write the report: {error}"))?;
    drop(stdout);

    summarise(&report);
    Ok(scan::exit_code(
        &report,
        arguments.fail_on,
        arguments.strict,
    ))
}

fn scan_stdin(options: &Options) -> Result<FileReport, String> {
    // Read bytes rather than a String: a document arriving on a pipe can
    // be UTF-16 as easily as a file can, and `read_to_string` would turn
    // that into an I/O error instead of the named refusal it is.
    let mut bytes = Vec::new();
    std::io::stdin()
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read stdin: {error}"))?;
    Ok(scan::scan_bytes(&bytes, "<stdin>".to_string(), options))
}

fn parse(args: &[String]) -> Result<Arguments, String> {
    let mut arguments = Arguments {
        inputs: Vec::new(),
        stdin: false,
        strict: false,
        fail_on: FailOn::default(),
        detect: Options::default(),
        walk: WalkOptions::default(),
    };

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        // Strict parsing, never a silent default: a typo'd `--strict`
        // that quietly did nothing would produce a report the caller
        // believed had insisted on coverage.
        if arg.starts_with('-') && !FLAGS.contains(&arg.as_str()) {
            return Err(format!("{arg} is not an option. Try --help."));
        }

        match arg.as_str() {
            "--stdin" => arguments.stdin = true,
            "--strict" => arguments.strict = true,
            "--hidden" => arguments.walk.hidden = true,
            "--no-ignore" => arguments.walk.respect_ignore = false,
            "--kind" => {
                for token in values(&mut rest, "--kind")? {
                    arguments.detect.kinds.push(detect::parse_kind(&token)?);
                }
            }
            "--script" => {
                for token in values(&mut rest, "--script")? {
                    arguments
                        .detect
                        .expected_scripts
                        .push(detect::scripts::parse_script(&token)?);
                }
            }
            "--fail-on" => {
                let value = values(&mut rest, "--fail-on")?.join(",");
                arguments.fail_on = fail_on(&value)?;
            }
            path => arguments.inputs.push(PathBuf::from(path)),
        }
    }

    if arguments.stdin && !arguments.inputs.is_empty() {
        return Err("reading from stdin takes no file arguments".to_string());
    }
    if !arguments.stdin && arguments.inputs.is_empty() {
        return Err("name a file or a directory to scan. Try --help.".to_string());
    }
    Ok(arguments)
}

/// The comma-separated values after a flag. Empty is a refusal rather
/// than a no-op: `--kind ""` asking for nothing is a typo every time.
fn values<'a>(
    rest: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> Result<Vec<String>, String> {
    let raw = rest.next().ok_or_else(|| format!("{flag} needs a value"))?;
    let values: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if values.is_empty() {
        return Err(format!("{flag} needs a value"));
    }
    Ok(values)
}

fn fail_on(value: &str) -> Result<FailOn, String> {
    match value {
        "any" => Ok(FailOn::Any),
        "bidi" => Ok(FailOn::Bidi),
        other => Err(format!(
            "{other} is not something to fail on; one of: {}",
            FAIL_ON.join(", ")
        )),
    }
}

/// The human half. Every line restates something already in the JSON,
/// except the hint about naming a script — which belongs to this surface
/// only, because the other one has no flags.
fn summarise(report: &Report) {
    let mut stderr = std::io::stderr().lock();

    for file in &report.files {
        for refusal in &file.refusals {
            let _ = writeln!(stderr, "{}: {}", escape::text(&file.file), refusal.detail);
        }
        for finding in &file.findings {
            let _ = writeln!(stderr, "{}", scan::describe(file, finding));
        }
    }

    let _ = writeln!(
        stderr,
        "{} in {}{}",
        plural(report.summary.findings, "finding", "findings"),
        plural(report.summary.files, "file", "files"),
        if report.summary.refusals == 0 {
            String::new()
        } else {
            format!(
                ", {} refused ({} not read at all)",
                report.summary.refusals, report.summary.unexamined
            )
        }
    );

    if report
        .files
        .iter()
        .flat_map(|file| &file.refusals)
        .any(|refusal| refusal.reason == detect::Reason::IntentionalScriptContext)
    {
        let _ = writeln!(
            stderr,
            "name the expected script with --script to have those files judged too"
        );
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Kind;

    #[test]
    fn every_documented_flag_is_parsed_and_the_reverse() {
        let mut documented: Vec<&str> = USAGE
            .split_whitespace()
            .filter(|word| word.starts_with("--"))
            .map(|word| word.trim_end_matches([',', '.', ':', ';']))
            .filter(|word| !matches!(*word, "--version" | "--help"))
            .collect();
        documented.sort_unstable();
        documented.dedup();

        let mut implemented = FLAGS.to_vec();
        implemented.sort_unstable();
        assert_eq!(documented, implemented);
    }

    #[test]
    fn the_parser_accepts_every_flag_it_lists() {
        for flag in FLAGS {
            let args: Vec<String> = match flag {
                "--kind" => vec![flag.into(), "bidi".into(), "x".into()],
                "--script" => vec![flag.into(), "Han".into(), "x".into()],
                "--fail-on" => vec![flag.into(), "any".into(), "x".into()],
                "--stdin" => vec![flag.into()],
                _ => vec![flag.into(), "x".into()],
            };
            assert!(parse(&args).is_ok(), "{flag}");
        }
    }

    /// Every kind the tool can report must be nameable from the command
    /// line and documented — otherwise a finding exists that no caller
    /// can filter to or filter out.
    #[test]
    fn every_kind_is_documented_and_accepted() {
        for (kind, _, short) in detect::KINDS {
            assert!(USAGE.contains(short), "{short} is undocumented");
            let parsed = parse(&["--kind".into(), short.into(), "x".into()]).expect(short);
            assert_eq!(parsed.detect.kinds, [kind]);
        }
    }

    #[test]
    fn every_documented_fail_on_value_is_accepted() {
        for value in FAIL_ON {
            assert!(fail_on(value).is_ok(), "{value}");
            assert!(USAGE.contains(value), "{value} is undocumented");
        }
    }

    #[test]
    fn kinds_and_scripts_may_be_listed_or_repeated() {
        let listed =
            parse(&["--kind".into(), "bidi,invisible".into(), "x".into()]).expect("parses");
        assert_eq!(listed.detect.kinds, [Kind::BidiControl, Kind::Invisible]);

        let repeated = parse(&[
            "--script".into(),
            "Han".into(),
            "--script".into(),
            "Cyrillic".into(),
            "x".into(),
        ])
        .expect("parses");
        assert_eq!(repeated.detect.expected_scripts.len(), 2);
    }

    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        let error = parse(&["--kinds".into(), "x".into()]).expect_err("a refusal");
        assert!(error.contains("--kinds"), "{error}");
    }

    #[test]
    fn a_flag_with_no_value_is_refused() {
        for flag in ["--kind", "--script", "--fail-on"] {
            assert!(parse(&[flag.into()]).is_err(), "{flag}");
            assert!(
                parse(&[flag.into(), String::new()]).is_err(),
                "{flag} empty"
            );
        }
    }

    #[test]
    fn an_unknown_kind_script_or_threshold_is_refused_by_name() {
        assert!(
            parse(&["--kind".into(), "homoglyph".into(), "x".into()])
                .expect_err("a refusal")
                .contains("homoglyph")
        );
        assert!(
            parse(&["--script".into(), "Klingon".into(), "x".into()])
                .expect_err("a refusal")
                .contains("Klingon")
        );
        assert!(
            fail_on("everything")
                .expect_err("a refusal")
                .contains("everything")
        );
    }

    #[test]
    fn naming_nothing_is_refused() {
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn stdin_and_file_arguments_together_are_refused() {
        assert!(parse(&["--stdin".into(), "x".into()]).is_err());
    }

    #[test]
    fn the_defaults_are_every_kind_no_script_and_fail_on_anything() {
        let arguments = parse(&["x".into()]).expect("parses");
        assert!(arguments.detect.kinds.is_empty());
        assert!(arguments.detect.expected_scripts.is_empty());
        assert_eq!(arguments.fail_on, FailOn::Any);
        assert!(!arguments.strict);
    }

    /// Nothing writes. If any of these is ever accepted, the line this
    /// tool was drawn along has moved.
    #[test]
    fn no_flag_offers_to_change_a_file() {
        for attempt in ["--fix", "--write", "--normalize", "--nfc", "--strip"] {
            assert!(
                parse(&[attempt.into(), "x".into()]).is_err(),
                "{attempt} was accepted"
            );
        }
    }

    /// **The regression.** `--help` prints USAGE to stdout with nothing
    /// between, and USAGE held an em dash — a non-ASCII byte on the
    /// protocol stream, from the one string in the output that nothing
    /// escapes. Every other path is covered: findings are `U+XXXX` and
    /// prose held to ASCII by `detect::hazards`, and the two fields this
    /// crate does not author go through `escape`.
    #[test]
    fn the_usage_text_is_ascii_because_help_prints_it_unescaped() {
        assert!(
            USAGE.is_ascii(),
            "USAGE reaches stdout verbatim: {}",
            USAGE
                .chars()
                .filter(|c| !c.is_ascii())
                .map(|c| format!("U+{:04X}", c as u32))
                .collect::<Vec<String>>()
                .join(" ")
        );
    }

    #[test]
    fn the_usage_text_states_what_it_will_not_do() {
        assert!(USAGE.contains("never rewrites"), "the non-goal is unstated");
        assert!(
            USAGE.contains("never the characters"),
            "the report-safety rule is unstated"
        );
        for code in ["0", "1", "2"] {
            assert!(USAGE.contains(code), "exit code {code} is undocumented");
        }
    }
}
