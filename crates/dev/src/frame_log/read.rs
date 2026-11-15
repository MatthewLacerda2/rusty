//! Reading the frame log back (#2206): the slice `Debug.FrameLog{ … }` returns, so
//! an agent never parses the files by hand.
//!
//! * **Newest first, stopping early.** Segments and their lines are read newest
//!   first; a look-back stops at the first line older than it, and a `limit`
//!   stops parsing once it is full (the lines past it are only counted, from
//!   their head, into `truncated`).
//! * **Filtered before parsed.** Each line's head ([`Head`]) says its channel,
//!   time, frame index and wall ms, so a line the query drops is never parsed.
//! * **GPU times joined by frame.** A frame's GPU times resolve a frame or two
//!   after it drew, so they ride on a later `frames` record under `gpu_frame`, the
//!   index of the frame they measure. The read moves them onto that frame's own
//!   record (matching `frame`), so `over_ms`' heaviest pass is the hitching
//!   frame's. A drawn frame whose times are not in the log (not resolved yet, or
//!   never) says `gpu_resolved = false` rather than borrowing a neighbour's.
//! * **A bad line is counted, never fatal.** A line that does not parse, or is
//!   not UTF-8 (a segment cut inside a character), is skipped and counted.

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use serde_json::{json, Map, Value};

use super::head::Head;
use super::segments::numbered;

/// The records a read keeps when it names neither a `limit` nor a look-back: the
/// newest ten seconds of a 60 Hz window, what a table returned to Lua holds
/// comfortably. A look-back is returned whole unless a `limit` is given.
pub const DEFAULT_LIMIT: usize = 600;

/// The fields a frame's GPU times travel in.
const GPU_FIELDS: [&str; 2] = ["gpu_ms", "gpu_passes"];

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

/// A later `frames` line carrying an earlier frame's GPU times: its segment's
/// bytes and where in them it lies.
type Carrier = (Rc<Vec<u8>>, Range<usize>);

/// Read `dir`'s log as `query` asks, against the wall clock `now` (ms).
pub fn read(dir: &Path, query: &LogQuery, now: u64) -> LogRead {
    let since = query
        .last_seconds
        .map(|s| now.saturating_sub((s * 1000.0).max(0.0) as u64));
    let limit = query.limit();
    let mut out = LogRead::default();
    let mut newest_first = Vec::new();
    // Carriers by the frame they measure, for frames not reached yet.
    let mut carriers = BTreeMap::<u64, Carrier>::new();
    let (mut last_frame, mut seen_gpu) = (None::<u64>, false);
    'segments: for (_, path) in numbered(dir).into_iter().rev() {
        // lint: allow(stored-format) the frame log's format is `frame_log::record`'s
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let bytes = Rc::new(bytes);
        let base = bytes.as_ptr() as usize;
        for (raw, line) in lines_newest_first(&bytes) {
            if raw.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let Some(line) = line else {
                out.skipped += 1;
                continue;
            };
            let (head, parsed) = match Head::of(line) {
                Some(head) => (head, None),
                None => match serde_json::from_str::<Value>(line) {
                    Ok(Value::Object(record)) => (Head::from_map(&record), Some(record)),
                    _ => {
                        out.skipped += 1;
                        continue;
                    }
                },
            };
            if since.is_some_and(|since| head.t < since) {
                break 'segments;
            }
            if head.ch == "dropped" {
                let records = |r: &Map<String, Value>| r.get("records").and_then(Value::as_u64);
                let n = match &parsed {
                    Some(record) => records(record),
                    None => parse(line).as_ref().and_then(records),
                };
                out.dropped += n.unwrap_or(0);
            }
            if let Some(f) = head.frame {
                if last_frame.is_some_and(|l| f > l) {
                    carriers.clear(); // an earlier session's frames: indices restart
                }
                // A carrier whose frame was passed unseen (dropped) is let go.
                while carriers.last_key_value().is_some_and(|(m, _)| *m > f) {
                    carriers.pop_last();
                }
                last_frame = Some(f);
            }
            if let Some(m) = head.gpu_frame {
                let start = raw.as_ptr() as usize - base;
                carriers.insert(m, (Rc::clone(&bytes), start..start + raw.len()));
                seen_gpu = true;
            }
            // A drawn frame takes its own times, once; a frame that drew nothing
            // shares the last drawn one's index and takes none. A line with no
            // frame index (not the writer's) keeps its own.
            let drew = head.cpu_ms.is_some();
            let taken = head
                .frame
                .filter(|_| drew)
                .and_then(|f| carriers.remove(&f));
            let carrier = match head.frame {
                Some(_) => taken.as_ref().map(carried),
                None => Some(line),
            };
            if !wanted(&head, line, carrier, query) {
                continue;
            }
            if newest_first.len() >= limit {
                out.truncated += 1;
                continue;
            }
            let Some(mut record) = parsed.or_else(|| parse(line)) else {
                out.skipped += 1;
                continue;
            };
            if head.ch == "frames" && head.frame.is_some() {
                join(&mut record, carrier.and_then(parse));
            }
            if let Some(record) = kept(record, query) {
                newest_first.push(record);
            }
        }
    }
    if seen_gpu {
        for record in &mut newest_first {
            let frame = record.get("ch").and_then(Value::as_str) == Some("frames");
            let drew = record.contains_key("cpu_ms");
            if frame && drew && !record.contains_key("gpu_ms") {
                record.insert("gpu_resolved".into(), json!(false));
            }
        }
    }
    newest_first.reverse();
    out.records = newest_first.into_iter().map(Value::Object).collect();
    out
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

fn carried((bytes, range): &Carrier) -> &str {
    std::str::from_utf8(&bytes[range.clone()]).unwrap_or("")
}

fn parse(line: &str) -> Option<Map<String, Value>> {
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

/// Move a frame's own GPU times onto its record: drop the later frame's times
/// that rode on it, and take `carrier`'s (the line whose `gpu_frame` is this
/// frame), if any.
fn join(record: &mut Map<String, Value>, carrier: Option<Map<String, Value>>) {
    record.remove("gpu_frame");
    for field in GPU_FIELDS {
        record.remove(field);
    }
    let Some(mut carrier) = carrier else {
        return;
    };
    for field in GPU_FIELDS {
        if let Some(value) = carrier.remove(field) {
            record.insert(field.into(), value);
        }
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
