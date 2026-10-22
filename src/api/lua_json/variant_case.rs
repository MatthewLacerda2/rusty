//! src/api/lua_json/variant_case.rs — enum names from Lua are case-insensitive (#1216).
//!
//! Every hand-written name setter (`Light.SetType`, `Graphics.SetTonemap`, …) reads
//! its name in any case; a recipe or a UI look reaches its enums through serde
//! instead, whose derive matches a variant name exactly. [`decode`] closes the gap
//! for every type at once, without touching any of them — zimmer's patch and song
//! enums included, which rusty cannot annotate:
//!
//! 1. decode as-is: a document spelled canonically costs one plain `from_value`;
//! 2. on serde's `unknown variant` error, find the one variant the given name
//!    matches ignoring case; none (or two) and the original error stands, so an
//!    unknown name reads exactly as it always did;
//! 3. find *which* string in the document serde was decoding — the one that, swapped
//!    for a probe, makes the error name the probe — rewrite that string alone to the
//!    canonical name, and decode again.
//!
//! Only the failing string is rewritten, so a node id or a file name that happens to
//! share the miscased name keeps its spelling, and a stored file written back from
//! the decoded value carries the canonical one. It works whatever the enum's shape
//! (unit, externally or internally tagged, flattened), since all of them report an
//! unknown name through serde's one `unknown_variant` message.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// A string no author writes, swapped in to find the position serde failed at.
const PROBE: &str = "\u{1}rusty-variant-probe\u{1}";

/// Decode `json` into `T`, reading every enum variant name case-insensitively.
/// Errors are serde's, unchanged.
pub fn decode<T: DeserializeOwned>(mut json: Value) -> Result<T, String> {
    let mut first_error = None;
    loop {
        let error = match serde_json::from_value(json.clone()) {
            Ok(value) => return Ok(value),
            Err(e) => e.to_string(),
        };
        let fixed = unknown_variant(&error)
            .and_then(|(given, expected)| canonical(&given, &expected).map(|c| (given, c)))
            .is_some_and(|(given, canon)| rewrite_failing::<T>(&mut json, &given, canon));
        let first = first_error.get_or_insert(error);
        if !fixed {
            return Err(first.clone());
        }
    }
}

/// The name and the expected variants of serde's
/// "unknown variant `x`, expected `A` or `B`" (or "one of `A`, `B`, `C`").
fn unknown_variant(error: &str) -> Option<(String, Vec<&str>)> {
    const HEAD: &str = "unknown variant `";
    const TAIL: &str = "`, expected ";
    let rest = &error[error.find(HEAD)? + HEAD.len()..];
    let split = rest.find(TAIL)?;
    let expected = rest[split + TAIL.len()..].split('`').skip(1).step_by(2);
    Some((rest[..split].to_owned(), expected.collect()))
}

/// The one variant `given` names when case is ignored; `None` when none or several do,
/// or when it already matches one exactly (the failure is then something else).
fn canonical<'a>(given: &str, expected: &[&'a str]) -> Option<&'a str> {
    let mut matches = expected.iter().filter(|v| v.eq_ignore_ascii_case(given));
    let canon = *matches.next()?;
    (matches.next().is_none() && canon != given).then_some(canon)
}

/// Rewrite the string serde failed on — the one equal to `given` whose swap for
/// [`PROBE`] makes the error name the probe — to `canon`. False when none does.
fn rewrite_failing<T: DeserializeOwned>(json: &mut Value, given: &str, canon: &str) -> bool {
    for index in 0..count_strings(json, given) {
        let mut probed = json.clone();
        replace_nth(&mut probed, given, index, PROBE);
        let names_probe = serde_json::from_value::<T>(probed)
            .err()
            .and_then(|e| unknown_variant(&e.to_string()).map(|(name, _)| name == PROBE))
            .unwrap_or(false);
        if names_probe {
            replace_nth(json, given, index, canon);
            return true;
        }
    }
    false
}

/// How many string values (not keys) equal `text`.
fn count_strings(json: &Value, text: &str) -> usize {
    match json {
        Value::String(s) => usize::from(s == text),
        Value::Array(items) => items.iter().map(|v| count_strings(v, text)).sum(),
        Value::Object(map) => map.values().map(|v| count_strings(v, text)).sum(),
        _ => 0,
    }
}

/// Replace the `n`th string value equal to `text` (in [`count_strings`]'s order) with
/// `with`; `n` counts down as the walk passes earlier matches.
fn replace_nth(json: &mut Value, text: &str, n: usize, with: &str) {
    fn walk(json: &mut Value, text: &str, n: &mut usize, with: &str) -> bool {
        match json {
            Value::String(s) if s == text => {
                if *n == 0 {
                    *s = with.to_owned();
                    return true;
                }
                *n -= 1;
                false
            }
            Value::Array(items) => items.iter_mut().any(|v| walk(v, text, n, with)),
            Value::Object(map) => map.values_mut().any(|v| walk(v, text, n, with)),
            _ => false,
        }
    }
    walk(json, text, &mut { n }, with);
}

#[cfg(test)]
#[path = "variant_case_tests.rs"]
mod tests;
