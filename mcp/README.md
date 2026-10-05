# unicode-le-mcp

<p align="center">
  <a href="https://marketplace.visualstudio.com/items?itemName=nolindnaidoo.unicode-le">
    <img src="https://img.shields.io/badge/Install%20from-VS%20Code-blue?style=for-the-badge&logo=visualstudiocode" alt="Install from VS Code Marketplace" />
  </a>
  <a href="https://open-vsx.org/extension/nolindnaidoo/unicode-le">
    <img src="https://img.shields.io/open-vsx/dt/nolindnaidoo/unicode-le?style=for-the-badge&label=Open%20VSX&color=blue" alt="Open VSX downloads" />
  </a>
  <a href="https://www.npmjs.com/package/unicode-le-mcp">
    <img src="https://img.shields.io/npm/v/unicode-le-mcp?style=for-the-badge&label=MCP%20server&color=blue&logo=npm" alt="unicode-le-mcp on npm" />
  </a>
  <a href="https://letools.dev/tools/unicode-le">
    <img src="https://img.shields.io/badge/LE%20Tools-letools.dev-blue?style=for-the-badge" alt="LE Tools" />
  </a>
</p>

An [MCP](https://modelcontextprotocol.io) server that finds the Unicode risks
in a document — the characters that hide meaning: bidirectional controls,
invisibles, homoglyphs, words that mix scripts, text that is not in NFC — the
detection engine behind the [Unicode-LE](https://letools.dev/tools/unicode-le)
editor extension, exposed as a tool an agent can call.

It reports codepoints as `U+XXXX` and **never returns the characters
themselves**. A model that pasted a document into its own reasoning has already
been handed the bidi controls in it; what comes back from here cannot carry them
on into a commit message or a review comment. Every non-ASCII character in a
reply leaves as a `\uXXXX` escape, including the key paths, which are text from
the document.

No dependencies, no network calls, no filesystem access. Content goes in,
structured results come out.

## Use it

Point any MCP host at `npx unicode-le-mcp`.

**Claude Code**

```bash
claude mcp add unicode-le -- npx -y unicode-le-mcp
```

**Anything with a JSON config** — Cursor, Windsurf, Claude Desktop:

```json
{
  "mcpServers": {
    "unicode-le": {
      "command": "npx",
      "args": ["-y", "unicode-le-mcp"]
    }
  }
}
```

**VS Code** needs nothing here. Install the extension instead — it
carries this server and registers it for you:
[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=nolindnaidoo.unicode-le)
· [Open VSX](https://open-vsx.org/extension/nolindnaidoo/unicode-le)

**No Node?** The same `detect_unicode_risks` tool ships in a static Rust
binary: `cargo install unicode-le`, then `unicode-le mcp`
([crates.io](https://crates.io/crates/unicode-le)). The two servers answer
identically — one corpus runs against both, a differential test feeds both
thousands of generated documents and compares every answer, and both read the
same Unicode tables, written out by the crate, rather than whatever version the
host's JavaScript engine carries. The binary additionally offers
`unicode_le_scan`, which walks a tree; **this server reads no files**.

Prefer a global install to `npx` on every launch:

```bash
npm install -g unicode-le-mcp
```

```json
{
  "mcpServers": {
    "unicode-le": { "command": "unicode-le-mcp" }
  }
}
```

No environment variables, no API key, no configuration of its own. To check it
before wiring it into anything:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | npx -y unicode-le-mcp
```

If that prints the tool name, the server works.

## The tool

### `detect_unicode_risks`

| argument | type | |
|---|---|---|
| `content` | string | **required.** The document text to scan. |
| `kinds` | string[] | Report only these: `bidi`, `invisible`, `confusable`, `mixed-script`, `non-nfc`, `whitespace`, `unassigned`. Omit for all of them — the only setting under which an empty result means clean. |
| `scripts` | string[] | Non-Latin scripts the document is expected to be written in, by Unicode name or ISO 15924 tag: `Han`, `Cyrillic`, `Hira`. Naming them lets the homoglyph check run on a translated document. |
| `format` | string | `json`, `yaml`, `toml`, `ini`, `env`, `csv`, `tsv` or `text`, so each finding can carry the key path it sits under. |
| `filename` | string | An alternative to `format`; only its extension is read. |
| `maxResults` | number | Default `500`, ceiling `5000`. |

Each finding carries its kind, severity, 1-based line and column (UTF-16, as an
editor counts), byte offset, the codepoints, the script and, where the format
allows, the key path:

```json
{
  "ok": true,
  "data": {
    "findings": [
      {
        "kind": "confusable",
        "severity": "high",
        "line": 2,
        "column": 8,
        "offset": 70,
        "key": "auth.provider",
        "codepoints": ["U+0430"],
        "scripts": ["Cyrillic"],
        "resembles": ["U+0061"],
        "detail": "a Cyrillic character in a word that is not Cyrillic, and it reduces to the codepoint under `resembles`: the two are indistinguishable on screen"
      }
    ],
    "refusals": []
  },
  "diagnostics": [],
  "meta": { "tool": "detect_unicode_risks", "count": 1, "truncated": false }
}
```

A document plainly written in a script nobody declared is **refused** for the
homoglyph and mixed-script checks rather than judged by them. The refusal comes
back twice — in `data.refusals` and as a `warning` diagnostic — so an empty
finding list is never mistaken for a clean document. `ok` means the check ran,
not that the answer was yes.

## Also in the MCP registry

`io.github.nolindnaidoo/unicode-le` —
[registry.modelcontextprotocol.io](https://registry.modelcontextprotocol.io)

## Ten more like it

One tool each, same shape: content in, structured data out, no network and no
filesystem. Every one is on npm as `<name>-mcp` and in the MCP registry as
`io.github.nolindnaidoo/<name>`.

| Package | Tool | Does |
|---|---|---|
| [`urls-le-mcp`](https://www.npmjs.com/package/urls-le-mcp) | `extract_urls` | URLs, with protocol and position |
| [`colors-le-mcp`](https://www.npmjs.com/package/colors-le-mcp) | `extract_colors` | colors from stylesheets and code |
| [`dates-le-mcp`](https://www.npmjs.com/package/dates-le-mcp) | `extract_dates` | dates and timestamps |
| [`numbers-le-mcp`](https://www.npmjs.com/package/numbers-le-mcp) | `extract_numbers` | numeric values |
| [`paths-le-mcp`](https://www.npmjs.com/package/paths-le-mcp) | `extract_paths` | file and directory paths |
| [`string-le-mcp`](https://www.npmjs.com/package/string-le-mcp) | `extract_strings` | string values |
| [`regex-le-mcp`](https://www.npmjs.com/package/regex-le-mcp) | `extract_patterns` | regexes, with a ReDoS verdict |
| [`secrets-le-mcp`](https://www.npmjs.com/package/secrets-le-mcp) | `detect_secrets` | credentials, masked — never the value |
| [`envsync-le-mcp`](https://www.npmjs.com/package/envsync-le-mcp) | `compare_env_files` | dotenv key drift, names only |
| [`scrape-le-mcp`](https://www.npmjs.com/package/scrape-le-mcp) | `analyze_robots_txt` | whether a path may be crawled |

Every tool in the family, one page: **[letools.dev](https://letools.dev)**

## Built by

**[Nolin Naidoo](https://nolindnaidoo.com)** — Chief Engineer, AI/ML & Platform
Architecture. [nolindnaidoo.com](https://nolindnaidoo.com) ·
[GitHub](https://github.com/nolindnaidoo) ·
[LinkedIn](https://www.linkedin.com/in/nolindnaidoo/)

### Also from the same workshop

Twelve Rust tools built the same way: small, single-purpose, and driven by a
machine rather than a person. pixelcoords and pixelactions make up one loop —
pixelcoords answers *where*, pixelactions *acts* there. The ten LE crates are
the terminal half of the extensions they sit in: the same detection, held to
the extension's own corpus, and an exit code instead of a results editor.

| | | |
|---|---|---|
| **[pixelcoords](https://github.com/nolindnaidoo/pixelcoords)** | Freeze your screen, mark regions, get pixel-exact coordinates and crops | [site](https://pixelcoords.dev) · [crates.io](https://crates.io/crates/pixelcoords) · [docs.rs](https://docs.rs/pixelcoords) |
| **[pixelactions](https://github.com/nolindnaidoo/pixelactions)** | Consume human-verified coordinates, perform the interaction, confirm it landed | [site](https://pixelactions.dev) · [crates.io](https://crates.io/crates/pixelactions) · [docs.rs](https://docs.rs/pixelactions) |
| **[paths-le](https://github.com/nolindnaidoo/paths-le/tree/main/crate)** | Find every path in a codebase and report whether it still points at anything | [crates.io](https://crates.io/crates/paths-le) |
| **[secrets-le](https://github.com/nolindnaidoo/secrets-le/tree/main/crate)** | Find hardcoded credentials, and never print one | [crates.io](https://crates.io/crates/secrets-le) |
| **[urls-le](https://github.com/nolindnaidoo/urls-le/tree/main/crate)** | Extract every URL from a codebase, with its protocol and exact position | [crates.io](https://crates.io/crates/urls-le) |
| **[regex-le](https://github.com/nolindnaidoo/regex-le/tree/main/crate)** | Find every regex in a codebase and report which can be driven into catastrophic backtracking | [crates.io](https://crates.io/crates/regex-le) |
| **[string-le](https://github.com/nolindnaidoo/string-le/tree/main/crate)** | Get every string in a codebase out where a person can read them | [crates.io](https://crates.io/crates/string-le) |
| **[numbers-le](https://github.com/nolindnaidoo/numbers-le/tree/main/crate)** | Find every hardcoded number in a codebase so a person can check them | [crates.io](https://crates.io/crates/numbers-le) |
| **[envsync-le](https://github.com/nolindnaidoo/envsync-le/tree/main/crate)** | Compare the dotenv files in a tree and say which keys are missing from which | [crates.io](https://crates.io/crates/envsync-le) |
| **[colors-le](https://github.com/nolindnaidoo/colors-le/tree/main/crate)** | Find every colour in a codebase, and say which are not in your palette | [crates.io](https://crates.io/crates/colors-le) |
| **[dates-le](https://github.com/nolindnaidoo/dates-le/tree/main/crate)** | Extract every date and timestamp, and the exact instant each one resolves to | [crates.io](https://crates.io/crates/dates-le) |
| **[scrape-le](https://github.com/nolindnaidoo/scrape-le/tree/main/crate)** | Check whether a page is scrapeable before the scraper is written | [crates.io](https://crates.io/crates/scrape-le) |

## Licence

MIT © [Nolin Naidoo](https://nolindnaidoo.com)
