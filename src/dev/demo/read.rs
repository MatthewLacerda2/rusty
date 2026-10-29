//! Reading a `.demo` back: the header, then every tick in order (#1581).
//!
//! [`Demo::ticks`] yields each tick from 0 with its whole input frame rebuilt, its
//! host acts, the Game view size and the digest when one was stored: the reader
//! a replay steps through, and what a search over a demo reads.

use std::path::Path;

use serde_json::Value;

use super::format::{host_call, Entry, Header};
use crate::core::input::InputFrame;
use crate::scripting::HostCall;

/// A demo read from disk.
pub struct Demo {
    pub header: Header,
    entries: Vec<Entry>,
    /// How many ticks it holds: its end line's count, or (cut short by a crash)
    /// one past its last line.
    pub ticks: u64,
}

/// One tick of a demo, whole.
#[derive(Clone, Debug, Default)]
pub struct DemoTick {
    pub t: u64,
    pub input: InputFrame,
    /// The host acts made before this tick, in order.
    pub host: Vec<HostCall>,
    /// The Game view's size from this tick on.
    pub size: Option<(u32, u32)>,
    /// The state digest stored after this tick, if one was.
    pub digest: Option<String>,
}

impl Demo {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read demo {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("demo {}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header = lines.next().ok_or("empty file")?;
        let header: Header = serde_json::from_str(header).map_err(|e| format!("header: {e}"))?;
        let (mut entries, mut end) = (Vec::<Entry>::new(), None);
        for (n, line) in lines.enumerate() {
            let value: Value =
                serde_json::from_str(line).map_err(|e| format!("line {}: {e}", n + 2))?;
            if let Some(ticks) = value.get("end").and_then(Value::as_u64) {
                end = Some(ticks);
                break;
            }
            let entry: Entry =
                serde_json::from_value(value).map_err(|e| format!("line {}: {e}", n + 2))?;
            if entries.last().is_some_and(|last| last.t >= entry.t) {
                return Err(format!("line {}: tick {} out of order", n + 2, entry.t));
            }
            entries.push(entry);
        }
        let ticks = end.unwrap_or_else(|| entries.last().map_or(0, |e| e.t + 1));
        Ok(Self {
            header,
            entries,
            ticks,
        })
    }

    /// Every tick from 0, in order.
    pub fn ticks(&self) -> DemoTicks<'_> {
        DemoTicks {
            demo: self,
            next: 0,
            entry: 0,
            tick: DemoTick {
                size: self.header.size,
                ..DemoTick::default()
            },
        }
    }
}

/// The ticks of a [`Demo`], rebuilt one by one from its deltas.
pub struct DemoTicks<'a> {
    demo: &'a Demo,
    next: u64,
    entry: usize,
    /// The last tick yielded; the next starts from its input and size.
    tick: DemoTick,
}

impl Iterator for DemoTicks<'_> {
    type Item = DemoTick;

    fn next(&mut self) -> Option<DemoTick> {
        if self.next >= self.demo.ticks {
            return None;
        }
        let t = self.next;
        self.next += 1;
        self.tick.t = t;
        self.tick.host.clear();
        self.tick.digest = None;
        if let Some(entry) = self.demo.entries.get(self.entry).filter(|e| e.t == t) {
            self.entry += 1;
            self.tick.input = entry.input.apply(&self.tick.input);
            self.tick.host = entry.host.iter().filter_map(host_call).collect();
            self.tick.size = entry.size.or(self.tick.size);
            self.tick.digest.clone_from(&entry.digest);
        }
        Some(self.tick.clone())
    }
}
