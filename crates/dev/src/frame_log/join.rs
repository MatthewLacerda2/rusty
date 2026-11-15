//! A frame's GPU times put back on its own record (#2206). They resolve a frame
//! or two after it drew and ride on a later `frames` line under `gpu_frame`; a
//! read, newest line first, meets that carrier before the frame it measures,
//! holds it here, and hands it over when it reaches that frame (`read`).

use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;

use serde_json::{Map, Value};

use super::head::Head;

/// The fields a frame's GPU times travel in.
const GPU_FIELDS: [&str; 2] = ["gpu_ms", "gpu_passes"];

/// A later `frames` line carrying an earlier frame's GPU times: its segment's
/// bytes and where in them it lies.
pub(super) type Carrier = (Rc<Vec<u8>>, Range<usize>);

/// The GPU carriers met so far, by the frame they measure, until it is reached.
#[derive(Default)]
pub(super) struct Carriers {
    by_frame: BTreeMap<u64, Carrier>,
    last_frame: Option<u64>,
    /// Whether the log had any GPU times at all.
    pub(super) seen: bool,
}

impl Carriers {
    /// Note `head`'s line (at `at`) and return the carrier of its frame's own
    /// GPU times when it drew: each is taken once, and a frame that drew nothing
    /// (sharing the last drawn one's index) takes none.
    pub(super) fn see(&mut self, head: &Head<'_>, at: Carrier) -> Option<Carrier> {
        if let Some(f) = head.frame {
            if self.last_frame.is_some_and(|l| f > l) {
                self.by_frame.clear(); // an earlier session's frames: indices restart
            }
            // A carrier whose frame was passed unseen (dropped) is let go.
            while self.by_frame.last_key_value().is_some_and(|(m, _)| *m > f) {
                self.by_frame.pop_last();
            }
            self.last_frame = Some(f);
        }
        if let Some(m) = head.gpu_frame {
            self.by_frame.insert(m, at);
            self.seen = true;
        }
        let drew = head.cpu_ms.is_some();
        let f = head.frame.filter(|_| drew)?;
        self.by_frame.remove(&f)
    }
}

pub(super) fn carried((bytes, range): &Carrier) -> &str {
    std::str::from_utf8(&bytes[range.clone()]).unwrap_or("")
}

/// Move a frame's own GPU times onto its record: drop the later frame's times
/// that rode on it, and take `carrier`'s (the line whose `gpu_frame` is this
/// frame), if any.
pub(super) fn join(record: &mut Map<String, Value>, carrier: Option<Map<String, Value>>) {
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
