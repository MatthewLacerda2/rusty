//! `log-tag`: an engine warning names where it came from (#1139).
//!
//! Engine `warn!` / `error!` records reach the console, the session's replies and the
//! MCP results beside Lua's own output (`src/logging`), so each message opens with
//! its module in brackets, `[Asset]`, `[Physics]`, as Unity's Console entries carry
//! their source. A `log::warn!(` or `log::error!(` in `src/` whose message literal
//! (on its line or the next, after an optional `target: "…",`) does not start with
//! `[` is a finding. A message that is not a literal says why with
//! `// lint: allow(log-tag) <why>`.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "log-tag";

const MACROS: &[&str] = &["log::warn!(", "log::error!("];

/// Every engine warning or error whose message does not open with `[Module]`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !path.starts_with("src/") {
        return Vec::new();
    }
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        let Some(at) = MACROS
            .iter()
            .find_map(|m| code.find(m).map(|at| at + m.len()))
        else {
            continue;
        };
        let next = lines.get(i + 1).copied().unwrap_or_default();
        if !tagged(&format!("{} {}", &code[at..], next.trim())) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "an engine warning opens with its module, `[Asset] …`".to_string(),
            });
        }
    }
    found
}

/// Does the macro's argument text open with a message literal starting `"[`?
fn tagged(args: &str) -> bool {
    let mut rest = args.trim_start();
    if let Some(after) = rest.strip_prefix("target:") {
        // Skip the target literal and its comma.
        let after = after.trim_start().strip_prefix('"').unwrap_or(after);
        rest = after.split_once('"').map_or("", |(_, r)| r);
        rest = rest
            .trim_start()
            .strip_prefix(',')
            .unwrap_or(rest)
            .trim_start();
    }
    rest.starts_with("\"[")
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "log_tag_tests.rs"]
mod tests;
