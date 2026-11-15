//! Reading the frame log back (#2206): the slice `Debug.FrameLog{ … }` returns, so
//! an agent never parses the files by hand.
//!
//! Segments are read newest first and stop once one starts before the look-back,
//! so `last_seconds = 60` reads a segment or two, not the whole budget. A line
//! that does not parse (the last one, cut by a power cut or still being written)
//! is skipped and counted.

use std::path::Path;

use serde_json::{json, Map, Value};

use super::writer::numbered;

/// The records a read keeps when it names no `limit`: the newest ten seconds of a
/// 60 Hz window, what a table returned to Lua holds comfortably.
pub const DEFAULT_LIMIT: usize = 600;

/// What to read: each filter optional, all of them applied.
#[derive(Clone, Debug, PartialEq)]
pub struct LogQuery {
    /// Only records from the last this many seconds, by the wall clock.
    pub last_seconds: Option<f64>,
    /// Only frame records slower than this (`ms`, the frame's wall time), each
    /// with its heaviest pass and system named.
    pub over_ms: Option<f64>,
    /// Only records of this channel; `gpu_passes` and `systems` keep the frame
    /// records that carry them.
    pub channel: Option<String>,
    /// At most this many records, the newest.
    pub limit: usize,
}

impl Default for LogQuery {
    fn default() -> Self {
        Self {
            last_seconds: None,
            over_ms: None,
            channel: None,
            limit: DEFAULT_LIMIT,
        }
    }
}

/// What a read found: the records oldest first, and the lines it skipped.
#[derive(Debug, Default, PartialEq)]
pub struct LogRead {
    pub records: Vec<Value>,
    pub skipped: usize,
}

/// Read `dir`'s log as `query` asks, against the wall clock `now` (ms).
pub fn read(dir: &Path, query: &LogQuery, now: u64) -> LogRead {
    let since = query
        .last_seconds
        .map(|s| now.saturating_sub((s * 1000.0).max(0.0) as u64));
    let mut out = LogRead::default();
    let mut newest_first = Vec::new();
    for (_, path) in numbered(dir).into_iter().rev() {
        // lint: allow(stored-format) the frame log's format is `frame_log::record`'s
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut started_before = false;
        for line in text.lines().rev().filter(|l| !l.trim().is_empty()) {
            let Ok(Value::Object(record)) = serde_json::from_str::<Value>(line) else {
                out.skipped += 1;
                continue;
            };
            let t = record.get("t").and_then(Value::as_u64).unwrap_or(0);
            if since.is_some_and(|since| t < since) {
                started_before = true;
                continue;
            }
            if let Some(record) = kept(record, query) {
                newest_first.push(record);
            }
        }
        if started_before || newest_first.len() >= query.limit {
            break;
        }
    }
    newest_first.truncate(query.limit);
    newest_first.reverse();
    out.records = newest_first;
    out
}

/// The record if `query` keeps it, named its heaviest pass and system when it
/// was asked for as over a budget.
fn kept(mut record: Map<String, Value>, query: &LogQuery) -> Option<Value> {
    let ch = record.get("ch").and_then(Value::as_str).unwrap_or("");
    if let Some(want) = &query.channel {
        if ch != want && !record.contains_key(want.as_str()) {
            return None;
        }
    }
    if let Some(over) = query.over_ms {
        let ms = record.get("ms").and_then(Value::as_f64);
        if ch != "frames" || !ms.is_some_and(|ms| ms > over) {
            return None;
        }
        for (field, name) in [
            ("gpu_passes", "heaviest_pass"),
            ("systems", "heaviest_system"),
        ] {
            if let Some(heaviest) = heaviest(record.get(field)) {
                record.insert(name.into(), json!(heaviest));
            }
        }
    }
    Some(Value::Object(record))
}

/// The key of the largest number in an object of numbers.
fn heaviest(map: Option<&Value>) -> Option<String> {
    let map = map?.as_object()?;
    map.iter()
        .filter_map(|(k, v)| Some((k, v.as_f64()?)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k.clone())
}
