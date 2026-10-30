//! Reading a `.demo` back: the header, then every tick in order (#1581).
//!
//! [`Demo::read`] checks the whole file once and keeps only its header and end
//! line; [`Demo::ticks`] then streams it from disk, yielding each tick from 0 with
//! its whole input frame rebuilt, its `dt`, its host acts, the Game view size and
//! the digest when one was stored. Nothing holds a whole demo in memory: an hour
//! of play with a pasted clipboard would be gigabytes. It is the reader a replay
//! steps through, and what a search over a demo reads.

use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::format::{host_call, End, Entry, Header};
use super::index::IndexLine;
use crate::core::input::InputFrame;
use crate::dev::harness::FIXED_DT;
use crate::scripting::HostCall;

/// A demo on disk, checked: its header and end line, its ticks streamed on demand.
pub struct Demo {
    pub header: Header,
    path: PathBuf,
    /// How many ticks it holds: its end line's count, or (cut short by a crash)
    /// one past its last line.
    pub ticks: u64,
    /// The end line Stop wrote (`End::default()` for a demo cut short).
    pub end: End,
}

/// One tick of a demo, whole.
#[derive(Clone, Debug, Default)]
pub struct DemoTick {
    pub t: u64,
    /// The tick's `dt` in seconds.
    pub dt: f32,
    pub input: InputFrame,
    /// The host acts made before this tick, in order.
    pub host: Vec<HostCall>,
    /// The Game view's size from this tick on.
    pub size: Option<(u32, u32)>,
    /// The state digest stored after this tick, if one was.
    pub digest: Option<String>,
}

/// The lines after a demo's header: a tick's [`Entry`], an index line, the [`End`].
enum Line {
    Entry(Box<Entry>),
    Index(IndexLine),
    End(End),
}

fn lines_of(path: &Path) -> Result<Lines<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| format!("cannot read demo {}: {e}", path.display()))?;
    Ok(BufReader::new(file).lines())
}

fn parse_line(line: &str, n: usize) -> Result<Line, String> {
    let at = |e: serde_json::Error| format!("line {n}: {e}");
    let value: Value = serde_json::from_str(line).map_err(at)?;
    Ok(if value.get("end").is_some() {
        Line::End(serde_json::from_value(value).map_err(at)?)
    } else if value.get("at").is_some() {
        Line::Index(serde_json::from_value(value).map_err(at)?)
    } else {
        Line::Entry(Box::new(serde_json::from_value(value).map_err(at)?))
    })
}

impl Demo {
    /// Read the demo at `path`: its header, and every line checked in order.
    pub fn read(path: &Path) -> Result<Self, String> {
        Self::check(path).map_err(|e| format!("demo {}: {e}", path.display()))
    }

    fn check(path: &Path) -> Result<Self, String> {
        let mut lines = lines_of(path)?.enumerate();
        let mut next_line = || -> Result<Option<(usize, String)>, String> {
            for (n, line) in lines.by_ref() {
                let line = line.map_err(|e| format!("line {}: {e}", n + 1))?;
                if !line.trim().is_empty() {
                    return Ok(Some((n + 1, line)));
                }
            }
            Ok(None)
        };
        let (_, header) = next_line()?.ok_or("empty file")?;
        let header: Header = serde_json::from_str(&header).map_err(|e| format!("header: {e}"))?;
        let (mut last, mut end) = (None::<u64>, None);
        while let Some((n, line)) = next_line()? {
            match parse_line(&line, n)? {
                Line::End(found) => {
                    end = Some(found);
                    break;
                }
                Line::Entry(entry) if last.is_some_and(|t| t >= entry.t) => {
                    return Err(format!("line {n}: tick {} out of order", entry.t));
                }
                Line::Entry(entry) => last = Some(entry.t),
                Line::Index(_) => {}
            }
        }
        let ticks = end
            .as_ref()
            .map_or_else(|| last.map_or(0, |t| t + 1), |e| e.end);
        Ok(Self {
            header,
            path: path.to_path_buf(),
            ticks,
            end: end.unwrap_or_default(),
        })
    }

    /// The file it was read from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every index line in order (#1597), read from disk as it goes: what a search
    /// reads, never replaying anything.
    pub fn index(&self) -> impl Iterator<Item = IndexLine> {
        let lines = lines_of(&self.path).into_iter().flatten().enumerate();
        lines.skip(1).filter_map(|(n, line)| {
            // Serialized field order puts `at` first: a tick line is skipped unparsed.
            let line = line.ok().filter(|l| l.starts_with("{\"at\":"))?;
            match parse_line(&line, n + 1) {
                Ok(Line::Index(index)) => Some(index),
                _ => None,
            }
        })
    }

    /// Every tick from 0, in order, read from disk as it goes.
    pub fn ticks(&self) -> DemoTicks {
        let mut lines = lines_of(&self.path).ok();
        // The header was checked by `read`.
        if let Some(lines) = &mut lines {
            lines.next();
        }
        DemoTicks {
            lines,
            of: self.ticks,
            next: 0,
            line: 1,
            ahead: None,
            tick: DemoTick {
                dt: FIXED_DT,
                size: self.header.size,
                ..DemoTick::default()
            },
        }
    }
}

/// The ticks of a [`Demo`], rebuilt one by one from its deltas.
pub struct DemoTicks {
    lines: Option<Lines<BufReader<File>>>,
    of: u64,
    next: u64,
    /// The file's line number last read.
    line: usize,
    /// The next entry read, not yet reached.
    ahead: Option<Entry>,
    /// The last tick yielded; the next starts from its input, `dt` and size.
    tick: DemoTick,
}

impl DemoTicks {
    /// The next entry in the file, unless the end line or the file's end comes
    /// first. A line that no longer parses (the file changed since `read`) ends
    /// the stream, said in the log.
    fn read_entry(&mut self) -> Option<Entry> {
        let lines = self.lines.as_mut()?;
        loop {
            self.line += 1;
            let line = match lines.next()? {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => parse_line(&line, self.line),
                Err(e) => Err(e.to_string()),
            };
            return match line {
                Ok(Line::Entry(entry)) => Some(*entry),
                Ok(Line::Index(_)) => continue,
                Ok(Line::End(_)) => None,
                Err(e) => {
                    log::warn!("[Demo] {e}");
                    self.lines = None;
                    None
                }
            };
        }
    }
}

impl Iterator for DemoTicks {
    type Item = DemoTick;

    fn next(&mut self) -> Option<DemoTick> {
        if self.next >= self.of {
            return None;
        }
        let t = self.next;
        self.next += 1;
        if self.ahead.is_none() {
            self.ahead = self.read_entry();
        }
        self.tick.t = t;
        self.tick.host.clear();
        self.tick.digest = None;
        // A capture is an edge: it is published on its own tick only.
        self.tick.input.clipboard = None;
        if let Some(entry) = self.ahead.take_if(|e| e.t == t) {
            self.tick.input = entry.input.apply(&self.tick.input);
            if let Some(dt) = entry.dt {
                // An `f32` widened to `f64` narrows back exactly.
                self.tick.dt = dt.0 as f32;
            }
            self.tick.host = entry.host.iter().filter_map(host_call).collect();
            self.tick.size = entry.size.or(self.tick.size);
            self.tick.digest = entry.digest;
        }
        Some(self.tick.clone())
    }
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
