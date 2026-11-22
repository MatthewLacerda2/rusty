use std::fs::File;
use std::io::{BufRead, BufReader, Lines};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::format::{host_call, End, Entry, Header};
use super::index::IndexLine;
use crate::harness::FIXED_DT;
use rusty_core::input::InputFrame;
use rusty_script::scripting::HostCall;

pub struct Demo {
    pub header: Header,
    path: PathBuf,
    /// The end line's count or, for a demo cut short by a crash, one past its last tick line.
    pub ticks: u64,
    pub end: End,
}

#[derive(Clone, Debug, Default)]
pub struct DemoTick {
    pub t: u64,
    pub dt: f32,
    pub input: InputFrame,
    pub host: Vec<HostCall>,
    pub size: Option<(u32, u32)>,
    pub display_scale: Option<f32>,
    pub digest: Option<String>,
    pub loads: Vec<u32>,
    pub load_progress: Option<u8>,
}

enum Line {
    Entry(Box<Entry>),
    Index,
    BadIndex(String),
    End(End),
}

fn is_index(line: &str) -> bool {
    line.starts_with("{\"at\":")
}

fn lines_of(path: &Path) -> Result<Lines<BufReader<File>>, String> {
    let file = File::open(path).map_err(|e| format!("cannot read demo {}: {e}", path.display()))?;
    Ok(BufReader::new(file).lines())
}

fn parse_line(line: &str, n: usize) -> Result<Line, String> {
    let at = |e: serde_json::Error| format!("line {n}: {e}");
    let index = |parsed: Result<IndexLine, _>| {
        parsed.map_or_else(|e| Line::BadIndex(at(e)), |_| Line::Index)
    };
    if is_index(line) {
        return Ok(index(serde_json::from_str(line)));
    }
    let value: Value = serde_json::from_str(line).map_err(at)?;
    Ok(if value.get("end").is_some() {
        Line::End(serde_json::from_value(value).map_err(at)?)
    } else if value.get("at").is_some() {
        index(serde_json::from_value(value))
    } else {
        Line::Entry(Box::new(serde_json::from_value(value).map_err(at)?))
    })
}

impl Demo {
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
        let (mut last, mut end, mut warned) = (None::<u64>, None, false);
        while let Some((n, line)) = next_line()? {
            match parse_line(&line, n)? {
                Line::BadIndex(e) if !warned => {
                    log::warn!("[Demo] {}: an index line skipped, {e}", path.display());
                    warned = true;
                }
                Line::Index | Line::BadIndex(_) => {}
                Line::End(found) => {
                    end = Some(found);
                    break;
                }
                Line::Entry(entry) if last.is_some_and(|t| t >= entry.t) => {
                    return Err(format!("line {n}: tick {} out of order", entry.t));
                }
                Line::Entry(entry) => last = Some(entry.t),
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn index(&self) -> impl Iterator<Item = IndexLine> {
        index_of(&self.path, self.ticks)
    }

    pub fn ticks(&self) -> DemoTicks {
        let mut lines = lines_of(&self.path).ok();
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
                display_scale: self.header.display_scale,
                ..DemoTick::default()
            },
        }
    }
}

pub fn index_of(path: &Path, ticks: u64) -> impl Iterator<Item = IndexLine> {
    let lines = lines_of(path).into_iter().flatten();
    lines.skip(1).filter_map(move |line| {
        let line = line.ok().filter(|l| is_index(l))?;
        serde_json::from_str::<IndexLine>(&line)
            .ok()
            .filter(|index| index.at < ticks)
    })
}

pub struct DemoTicks {
    lines: Option<Lines<BufReader<File>>>,
    of: u64,
    next: u64,
    line: usize,
    ahead: Option<Entry>,
    tick: DemoTick,
}

impl DemoTicks {
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
                Ok(Line::Index | Line::BadIndex(_)) => continue,
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
        self.tick.loads.clear();
        self.tick.load_progress = None;
        self.tick.input.clipboard = None;
        if let Some(entry) = self.ahead.take_if(|e| e.t == t) {
            self.tick.input = entry.input.apply(&self.tick.input);
            if let Some(dt) = entry.dt {
                self.tick.dt = dt.0 as f32;
            }
            self.tick.host = entry.host.iter().filter_map(host_call).collect();
            self.tick.size = entry.size.or(self.tick.size);
            self.tick.display_scale = entry.display_scale.or(self.tick.display_scale);
            self.tick.digest = entry.digest;
            self.tick.loads = entry.loads;
            self.tick.load_progress = entry.load_progress;
        }
        Some(self.tick.clone())
    }
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
