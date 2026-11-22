//! A line's head, read without parsing it. `record::line` opens every line with `ch`, `t`
//! and `tick`, and a frame's with `frame`, `ms` and `cpu_ms`, in that order: change both
//! together. A line written another way is parsed whole (`from_map`).

use std::borrow::Cow;

use serde_json::{Map, Value};

#[derive(Debug, PartialEq)]
pub(super) struct Head<'a> {
    pub ch: Cow<'a, str>,
    pub t: u64,
    pub frame: Option<u64>,
    pub ms: Option<f64>,
    pub cpu_ms: Option<f64>,
}

impl<'a> Head<'a> {
    pub fn of(line: &'a str) -> Option<Self> {
        let rest = line.strip_prefix(r#"{"ch":""#)?;
        let (ch, mut rest) = rest.split_once('"')?;
        let t = scalar(&mut rest, "t")?.parse().ok()?;
        scalar(&mut rest, "tick")?;
        let mut head = Self {
            ch: Cow::Borrowed(ch),
            t,
            frame: None,
            ms: None,
            cpu_ms: None,
        };
        if ch == "frames" {
            head.frame = scalar(&mut rest, "frame").and_then(|v| v.parse().ok());
            head.ms = scalar(&mut rest, "ms").and_then(|v| v.parse().ok());
            head.cpu_ms = scalar(&mut rest, "cpu_ms").and_then(|v| v.parse().ok());
        }
        Some(head)
    }

    pub fn from_map(record: &Map<String, Value>) -> Self {
        let ch = record.get("ch").and_then(Value::as_str).unwrap_or("");
        Self {
            ch: Cow::Owned(ch.to_string()),
            t: record.get("t").and_then(Value::as_u64).unwrap_or(0),
            frame: record.get("frame").and_then(Value::as_u64),
            ms: record.get("ms").and_then(Value::as_f64),
            cpu_ms: record.get("cpu_ms").and_then(Value::as_f64),
        }
    }
}

fn scalar<'a>(rest: &mut &'a str, key: &str) -> Option<&'a str> {
    let after = rest
        .strip_prefix(",\"")?
        .strip_prefix(key)?
        .strip_prefix("\":")?;
    let end = after.find([',', '}']).unwrap_or(after.len());
    let value = &after[..end];
    *rest = &after[end..];
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_head_is_read_and_another_line_is_not() {
        let line = r#"{"ch":"frames","t":12,"tick":3,"frame":7,"ms":16.5,"cpu_ms":2,"cam":[0]}"#;
        let head = Head::of(line).unwrap();
        let want = (Some(7), Some(16.5), Some(2.0), 12);
        assert_eq!((head.frame, head.ms, head.cpu_ms, head.t), want);
        let console = Head::of(r#"{"ch":"console","t":1,"tick":0,"level":"info"}"#).unwrap();
        assert_eq!((console.ch.as_ref(), console.frame), ("console", None));
        let undrawn = Head::of(r#"{"ch":"frames","t":1,"tick":0,"frame":2,"ms":9}"#).unwrap();
        assert_eq!((undrawn.ms, undrawn.cpu_ms), (Some(9.0), None));
        let open = Head::of(r#"{"ch":"frames","t":1,"tick":0,"frame":2,"cpu_ms":1}"#).unwrap();
        assert_eq!((open.ms, open.cpu_ms), (None, Some(1.0)), "ms unresolved");
        assert!(Head::of(r#"{"cam":[0],"ch":"frames","t":1}"#).is_none());
        assert!(Head::of(r#"{"ch":"fra"#).is_none(), "a cut line");
    }
}
