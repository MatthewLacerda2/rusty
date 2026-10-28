//! src/core/naming/variant_case.rs — enum names read in any case (#1216).
//!
//! Every hand-written name setter (`Light.SetType`, `Graphics.SetTonemap`, …) reads
//! its name in any case; a recipe or a UI look reaches its enums through serde
//! instead, whose derive matches a variant name exactly (a `named_enum!` writes its
//! own serde, which already reads any case, #1478). [`decode`] closes the gap
//! for every type at once, without touching any of them — zimmer's patch and song
//! enums included, which rusty cannot annotate:
//!
//! 1. decode as-is: a document spelled canonically costs one plain decode, read
//!    from the `Value` in place;
//! 2. on serde's `unknown variant` error, find the one variant the given name
//!    matches ignoring case; none (or two) and that error stands, so an unknown
//!    name reads exactly as it always did;
//! 3. find *which* string in the document serde was decoding — the one that, swapped
//!    for a probe, makes serde fail on the probe against the same variant list —
//!    rewrite that string alone to the canonical name, and decode again;
//! 4. the error returned is the one that remains, so a real mistake later in the
//!    document is reported as itself, not as a miscased name already fixed.
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
/// An error is serde's for the document as it stands after the rewrites — plain
/// serde's own when nothing was rewritten.
pub fn decode<T: DeserializeOwned>(mut json: Value) -> Result<T, String> {
    let mut rewrites_left = None;
    loop {
        let error = match T::deserialize(&json) {
            Ok(value) => return Ok(value),
            Err(e) => e.to_string(),
        };
        // Each rewrite leaves one string canonical for good, so a document never
        // needs more rewrites than it has strings; the bound only guards that.
        let left = rewrites_left.get_or_insert_with(|| string_paths(&json, |_| true).len());
        if *left == 0 || !rewrite_failing::<T>(&mut json, &error) {
            return Err(error);
        }
        *left -= 1;
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

/// When `error` is an unknown variant that names one variant ignoring case, rewrite
/// the string serde failed on to that variant: the string equal to the given name
/// whose swap for [`PROBE`] makes serde fail on the probe, against the same list of
/// variants. False when no string does.
fn rewrite_failing<T: DeserializeOwned>(json: &mut Value, error: &str) -> bool {
    let Some((given, expected)) = unknown_variant(error) else {
        return false;
    };
    let Some(canon) = canonical(&given, &expected) else {
        return false;
    };
    for path in string_paths(json, |s| s == given) {
        set(json, &path, PROBE);
        let names_probe = T::deserialize(&*json).err().is_some_and(|e| {
            unknown_variant(&e.to_string()) == Some((PROBE.to_owned(), expected.clone()))
        });
        set(json, &path, if names_probe { canon } else { &given });
        if names_probe {
            return true;
        }
    }
    false
}

/// The JSON pointer of every string value (not key) that `keep` accepts.
fn string_paths(json: &Value, keep: impl Fn(&str) -> bool) -> Vec<String> {
    fn walk(json: &Value, at: &mut String, keep: &dyn Fn(&str) -> bool, out: &mut Vec<String>) {
        let mut child = |at: &mut String, key: &str, value: &Value| {
            let len = at.len();
            at.push('/');
            at.push_str(&key.replace('~', "~0").replace('/', "~1"));
            walk(value, at, keep, out);
            at.truncate(len);
        };
        match json {
            Value::String(s) if keep(s) => out.push(at.clone()),
            Value::Array(items) => {
                for (i, v) in items.iter().enumerate() {
                    child(at, &i.to_string(), v);
                }
            }
            Value::Object(map) => map.iter().for_each(|(k, v)| child(at, k, v)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(json, &mut String::new(), &keep, &mut out);
    out
}

/// Set the string at `path` (from [`string_paths`]) to `text`.
fn set(json: &mut Value, path: &str, text: &str) {
    if let Some(value) = json.pointer_mut(path) {
        *value = Value::String(text.to_owned());
    }
}

#[cfg(test)]
#[path = "variant_case_tests.rs"]
mod tests;
