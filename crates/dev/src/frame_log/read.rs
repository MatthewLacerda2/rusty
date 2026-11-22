use std::path::Path;

use serde_json::{json, Map, Value};

use super::head::Head;
use super::held;
use super::segments::numbered;

/// Records a read with neither a `limit` nor a look-back returns: ten seconds of a 60 Hz
/// window, a table Lua holds comfortably.
pub const DEFAULT_LIMIT: usize = 600;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogQuery {
    pub last_seconds: Option<f64>,
    pub over_ms: Option<f64>,
    pub channel: Option<String>,
    pub limit: Option<usize>,
}

impl LogQuery {
    fn limit(&self) -> usize {
        let all = self.last_seconds.is_some();
        self.limit
            .unwrap_or(if all { usize::MAX } else { DEFAULT_LIMIT })
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct LogRead {
    pub records: Vec<Value>,
    pub skipped: usize,
    pub truncated: usize,
    pub dropped: u64,
}

pub fn read(dir: &Path, query: &LogQuery, now: u64) -> LogRead {
    let mut reader = Reader {
        query,
        since: query
            .last_seconds
            .map(|s| now.saturating_sub((s * 1000.0).max(0.0) as u64)),
        limit: query.limit(),
        out: LogRead::default(),
        newest_first: Vec::new(),
        drops: Vec::new(),
        unplaced: 0,
    };
    let mut more = reader.segment(held::lines(dir).join("\n").as_bytes());
    for (_, path) in numbered(dir).into_iter().rev() {
        if !more {
            break;
        }
        // lint: allow(stored-format) the frame log's format is `frame_log::record`'s
        if let Ok(bytes) = std::fs::read(&path) {
            more = reader.segment(&bytes);
        }
    }
    if more {
        reader.out.skipped += reader.unplaced;
    }
    reader.finish()
}

struct Reader<'q> {
    query: &'q LogQuery,
    since: Option<u64>,
    limit: usize,
    out: LogRead,
    newest_first: Vec<Map<String, Value>>,
    drops: Vec<(u64, u64)>,
    /// Bad lines not yet known to lie inside the look-back.
    unplaced: usize,
}

impl Reader<'_> {
    fn segment(&mut self, bytes: &[u8]) -> bool {
        for (raw, line) in lines_newest_first(bytes) {
            if raw.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let Some((head, parsed)) = line.and_then(head_of) else {
                self.unplaced += 1;
                continue;
            };
            if self.since.is_some_and(|since| head.t < since) {
                return false;
            }
            self.out.skipped += std::mem::take(&mut self.unplaced);
            self.line(&head, line.unwrap_or_default(), parsed);
        }
        self.newest_first.len() < self.limit
    }

    fn line(&mut self, head: &Head<'_>, line: &str, parsed: Option<Record>) {
        let full = self.newest_first.len() >= self.limit;
        if head.ch == "dropped" && !full {
            let records = |r: &Record| r.get("records").and_then(Value::as_u64);
            let n = match &parsed {
                Some(record) => records(record),
                None => parse(line).as_ref().and_then(records),
            };
            self.drops.push((head.t, n.unwrap_or(0)));
        }
        if !wanted(head, line, self.query) {
            return;
        }
        if full {
            self.out.truncated += 1;
            return;
        }
        let Some(record) = parsed.or_else(|| parse(line)) else {
            self.out.skipped += 1;
            return;
        };
        if let Some(record) = kept(record, self.query) {
            self.newest_first.push(record);
        }
    }

    fn finish(mut self) -> LogRead {
        let t = |r: &Record| r.get("t").and_then(Value::as_u64);
        let oldest = self.newest_first.last().and_then(t);
        let full = self.out.truncated > 0;
        let from = match (oldest, self.since) {
            (Some(oldest), Some(since)) if !full => oldest.min(since),
            (Some(oldest), _) => oldest,
            (None, since) => since.unwrap_or(u64::MAX),
        };
        self.out.dropped = self
            .drops
            .iter()
            .filter(|(t, _)| *t >= from)
            .map(|(_, n)| n)
            .sum();
        self.newest_first.reverse();
        let records = self.newest_first.into_iter().map(Value::Object);
        self.out.records = records.collect();
        self.out
    }
}

type Record = Map<String, Value>;

fn head_of(line: &str) -> Option<(Head<'_>, Option<Record>)> {
    match Head::of(line) {
        Some(head) => Some((head, None)),
        None => parse(line).map(|record| (Head::from_map(&record), Some(record))),
    }
}

/// Splits as text, which finds line ends a word at a time, unless a cut inside a character
/// left the segment not UTF-8.
fn lines_newest_first(bytes: &[u8]) -> Box<dyn Iterator<Item = (&[u8], Option<&str>)> + '_> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Box::new(text.rsplit('\n').map(|l| (l.as_bytes(), Some(l)))),
        Err(_) => Box::new(
            bytes
                .rsplit(|b| *b == b'\n')
                .map(|l| (l, std::str::from_utf8(l).ok())),
        ),
    }
}

fn parse(line: &str) -> Option<Record> {
    match serde_json::from_str(line) {
        Ok(Value::Object(record)) => Some(record),
        _ => None,
    }
}

fn wanted(head: &Head<'_>, line: &str, query: &LogQuery) -> bool {
    if let Some(want) = query.channel.as_deref() {
        let ok = match want {
            "gpu_passes" | "systems" => {
                head.ch == "frames" && line.contains(&format!("\"{want}\":"))
            }
            _ => head.ch == want,
        };
        if !ok {
            return false;
        }
    }
    match query.over_ms {
        Some(over) => head.ch == "frames" && head.ms.is_some_and(|ms| ms > over),
        None => true,
    }
}

fn kept(mut record: Map<String, Value>, query: &LogQuery) -> Option<Map<String, Value>> {
    let ch = record.get("ch").and_then(Value::as_str).unwrap_or("");
    if let Some(want) = &query.channel {
        if ch != want && !record.contains_key(want.as_str()) {
            return None;
        }
    }
    if query.over_ms.is_some() {
        for (field, name) in [
            ("gpu_passes", "heaviest_pass"),
            ("systems", "heaviest_system"),
        ] {
            if let Some(heaviest) = heaviest(record.get(field)) {
                record.insert(name.into(), json!(heaviest));
            }
        }
    }
    Some(record)
}

fn heaviest(map: Option<&Value>) -> Option<String> {
    let map = map?.as_object()?;
    map.iter()
        .filter_map(|(k, v)| Some((k, v.as_f64()?)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k.clone())
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
