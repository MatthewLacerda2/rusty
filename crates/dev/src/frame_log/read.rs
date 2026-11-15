//! Reading the frame log back (#2206): the slice `Debug.FrameLog{ … }` returns, so
//! an agent never parses the files by hand.
//!
//! * **Newest first, stopping early.** Segments and their lines are read newest
//!   first; a look-back stops at the first line older than it, and a `limit`
//!   stops parsing once it is full (the lines past it are only counted, from
//!   their head, into `truncated`).
//! * **Filtered before parsed.** Each line's head (`head::Head`) says its channel,
//!   time, frame index and wall ms, so a line the query drops is never parsed.
//! * **GPU times joined by frame.** A frame's GPU times resolve a frame or two
//!   after it drew, so they ride on a later `frames` record under `gpu_frame`, the
//!   index of the frame they measure. The read moves them onto that frame's own
//!   record (matching `frame`), so `over_ms`' heaviest pass is the hitching
//!   frame's. A drawn frame whose times are not in the log (not resolved yet, or
//!   never) says `gpu_resolved = false` rather than borrowing a neighbour's.
//! * **A bad line is counted, never fatal.** A line that does not parse, or is
//!   not UTF-8 (a segment cut inside a character), is skipped and counted.

use std::path::Path;
use std::rc::Rc;

use serde_json::{json, Map, Value};

use super::head::Head;
use super::join::{carried, join, Carrier, Carriers};
use super::segments::numbered;

/// The records a read keeps when it names neither a `limit` nor a look-back: the
/// newest ten seconds of a 60 Hz window, what a table returned to Lua holds
/// comfortably. A look-back is returned whole unless a `limit` is given.
pub const DEFAULT_LIMIT: usize = 600;

/// What to read: each filter optional, all of them applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogQuery {
    /// Only records from the last this many seconds, by the wall clock.
    pub last_seconds: Option<f64>,
    /// Only frame records slower than this (`ms`, the frame's wall time), each
    /// with its heaviest pass and system named.
    pub over_ms: Option<f64>,
    /// Only records of this channel; `gpu_passes` and `systems` keep the frame
    /// records that carry them.
    pub channel: Option<String>,
    /// At most this many records, the newest; `None` is [`DEFAULT_LIMIT`] without
    /// a look-back and every record with one.
    pub limit: Option<usize>,
}

impl LogQuery {
    fn limit(&self) -> usize {
        let all = self.last_seconds.is_some();
        self.limit
            .unwrap_or(if all { usize::MAX } else { DEFAULT_LIMIT })
    }
}

/// What a read found: the records oldest first; the lines it skipped; the
/// records the query matched past its limit, left out; and the records the
/// writer dropped in the span read (the `dropped` lines' sum).
#[derive(Debug, Default, PartialEq)]
pub struct LogRead {
    pub records: Vec<Value>,
    pub skipped: usize,
    pub truncated: usize,
    pub dropped: u64,
}

/// Read `dir`'s log as `query` asks, against the wall clock `now` (ms).
pub fn read(dir: &Path, query: &LogQuery, now: u64) -> LogRead {
    let mut reader = Reader {
        query,
        since: query
            .last_seconds
            .map(|s| now.saturating_sub((s * 1000.0).max(0.0) as u64)),
        limit: query.limit(),
        out: LogRead::default(),
        newest_first: Vec::new(),
        carriers: Carriers::default(),
    };
    for (_, path) in numbered(dir).into_iter().rev() {
        // lint: allow(stored-format) the frame log's format is `frame_log::record`'s
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if !reader.segment(&Rc::new(bytes)) {
            break;
        }
    }
    reader.finish()
}

/// A read in progress, newest line first.
struct Reader<'q> {
    query: &'q LogQuery,
    since: Option<u64>,
    limit: usize,
    out: LogRead,
    newest_first: Vec<Map<String, Value>>,
    carriers: Carriers,
}

impl Reader<'_> {
    /// Read one segment's lines; `false` once a line is older than the look-back.
    fn segment(&mut self, bytes: &Rc<Vec<u8>>) -> bool {
        let base = bytes.as_ptr() as usize;
        for (raw, line) in lines_newest_first(bytes) {
            if raw.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let Some((head, parsed)) = line.and_then(head_of) else {
                self.out.skipped += 1;
                continue;
            };
            if self.since.is_some_and(|since| head.t < since) {
                return false;
            }
            let start = raw.as_ptr() as usize - base;
            let at = (Rc::clone(bytes), start..start + raw.len());
            let line = line.unwrap_or_default();
            self.line(&head, line, parsed, at);
        }
        true
    }

    /// One line in the look-back: counted, carried, or kept as `query` asks.
    fn line(&mut self, head: &Head<'_>, line: &str, parsed: Option<Record>, at: Carrier) {
        if head.ch == "dropped" {
            let records = |r: &Record| r.get("records").and_then(Value::as_u64);
            let n = match &parsed {
                Some(record) => records(record),
                None => parse(line).as_ref().and_then(records),
            };
            self.out.dropped += n.unwrap_or(0);
        }
        let taken = self.carriers.see(head, at);
        // A line with no frame index (not the writer's) keeps its own times.
        let carrier = match head.frame {
            Some(_) => taken.as_ref().map(carried),
            None => Some(line),
        };
        if !wanted(head, line, carrier, self.query) {
            return;
        }
        if self.newest_first.len() >= self.limit {
            self.out.truncated += 1;
            return;
        }
        let Some(mut record) = parsed.or_else(|| parse(line)) else {
            self.out.skipped += 1;
            return;
        };
        if head.ch == "frames" && head.frame.is_some() {
            join(&mut record, carrier.and_then(parse));
        }
        if let Some(record) = kept(record, self.query) {
            self.newest_first.push(record);
        }
    }

    /// The records oldest first; a drawn frame without its GPU times says so,
    /// when the log has GPU times at all.
    fn finish(mut self) -> LogRead {
        if self.carriers.seen {
            for record in &mut self.newest_first {
                let frame = record.get("ch").and_then(Value::as_str) == Some("frames");
                let drew = record.contains_key("cpu_ms");
                if frame && drew && !record.contains_key("gpu_ms") {
                    record.insert("gpu_resolved".into(), json!(false));
                }
            }
        }
        self.newest_first.reverse();
        let records = self.newest_first.into_iter().map(Value::Object);
        self.out.records = records.collect();
        self.out
    }
}

type Record = Map<String, Value>;

/// A line's head, read cheaply, or by parsing it (kept, so it is parsed once);
/// `None` when it does not parse.
fn head_of(line: &str) -> Option<(Head<'_>, Option<Record>)> {
    match Head::of(line) {
        Some(head) => Some((head, None)),
        None => parse(line).map(|record| (Head::from_map(&record), Some(record))),
    }
}

/// A segment's lines, newest first, each with its text when it is UTF-8. A
/// segment that is UTF-8 throughout (every one but a segment cut inside a
/// character) is split as text, which searches for line ends a word at a time.
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

/// Whether a line may be kept, from its head (and, for a channel that is a
/// field, the line's or its GPU carrier's text) alone: exact for every query.
fn wanted(head: &Head<'_>, line: &str, carrier: Option<&str>, query: &LogQuery) -> bool {
    if let Some(want) = query.channel.as_deref() {
        let field = |text: &str| text.contains(&format!("\"{want}\":"));
        let ok = match want {
            "gpu_passes" => head.ch == "frames" && carrier.is_some_and(field),
            "systems" => head.ch == "frames" && field(line),
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

/// The record if `query` keeps it, named its heaviest pass and system when it
/// was asked for as over a budget.
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

/// The key of the largest number in an object of numbers.
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
