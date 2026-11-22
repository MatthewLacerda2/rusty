//! [`decode`] reads enum variant names in any case for every serde type at once. On
//! serde's `unknown variant` error it finds the one variant the name matches ignoring
//! case, rewrites only the string serde failed on (found by swapping it for a probe) and
//! decodes again, so a node id sharing the name keeps its spelling and a later real
//! mistake is reported as itself.

use serde::de::DeserializeOwned;
use serde_json::Value;

const PROBE: &str = "\u{1}rusty-variant-probe\u{1}";

pub fn decode<T: DeserializeOwned>(mut json: Value) -> Result<T, String> {
    let mut rewrites_left = None;
    loop {
        let error = match T::deserialize(&json) {
            Ok(value) => return Ok(value),
            Err(e) => e.to_string(),
        };
        let left = rewrites_left.get_or_insert_with(|| string_paths(&json, |_| true).len());
        if *left == 0 || !rewrite_failing::<T>(&mut json, &error) {
            return Err(error);
        }
        *left -= 1;
    }
}

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

fn set(json: &mut Value, path: &str, text: &str) {
    if let Some(value) = json.pointer_mut(path) {
        *value = Value::String(text.to_owned());
    }
}

#[cfg(test)]
#[path = "variant_case_tests.rs"]
mod tests;
