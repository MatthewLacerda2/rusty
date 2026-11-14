//! Finding a moment in a demo (#1597): a query over its index, never a replay.
//!
//! `Harness.FindInDemo(query)` reads one of four kinds of query and returns the
//! ticks that match, in order, each once: a `Debug.Mark` by its label, a console
//! line by a piece of its text, an event by kind, or the active camera's sampled
//! position inside a box.

use std::path::Path;

use glam::Vec3;

use super::super::read::{index_of, Demo};
use super::{Event, IndexLine};

/// What `Harness.FindInDemo` looks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Query {
    /// A `Debug.Mark` whose label is this.
    Mark(String),
    /// A console line holding this text.
    Log(String),
    /// An event of this kind: `spawned`, `destroyed`, `scene` or `play`.
    Event(EventKind),
    /// A camera sample inside this box (`min` to `max`, edges included).
    Inside { min: Vec3, max: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Spawned,
    Destroyed,
    Scene,
    Play,
}

impl EventKind {
    /// The kind named `name`, or the names there are.
    pub fn named(name: &str) -> Result<Self, String> {
        Ok(match name {
            "spawned" => Self::Spawned,
            "destroyed" => Self::Destroyed,
            "scene" => Self::Scene,
            "play" => Self::Play,
            other => {
                return Err(format!(
                    "no event {other:?}: \"spawned\", \"destroyed\", \"scene\" or \"play\""
                ))
            }
        })
    }

    fn of(self, event: &Event) -> bool {
        matches!(
            (self, event),
            (Self::Spawned, Event::Spawned { .. })
                | (Self::Destroyed, Event::Destroyed { .. })
                | (Self::Scene, Event::Scene { .. })
                | (Self::Play, Event::Play { .. })
        )
    }
}

impl Query {
    /// Whether `line` matches.
    pub fn matches(&self, line: &IndexLine) -> bool {
        match self {
            Self::Inside { min, max } => line.camera.is_some_and(|at| {
                let at = Vec3::from_array(at);
                at.cmpge(*min).all() && at.cmple(*max).all()
            }),
            _ => line.events.iter().any(|event| self.event(event)),
        }
    }

    fn event(&self, event: &Event) -> bool {
        match (self, event) {
            (Self::Mark(want), Event::Mark { label, .. }) => label == want,
            (Self::Log(want), Event::Log { text, .. }) => text.contains(want.as_str()),
            (Self::Event(kind), event) => kind.of(event),
            _ => false,
        }
    }
}

impl Demo {
    /// The ticks whose index lines match `query`, in order, each once.
    pub fn find(&self, query: &Query) -> Vec<u64> {
        find_in(self.path(), self.ticks, query)
    }
}

/// [`Demo::find`] on the demo at `path`, which holds `ticks` ticks: what a replay
/// knows of its demo, without checking the file again.
pub fn find_in(path: &Path, ticks: u64, query: &Query) -> Vec<u64> {
    let mut found: Vec<u64> = index_of(path, ticks)
        .filter(|line| query.matches(line))
        .map(|line| line.at)
        .collect();
    found.dedup();
    found
}
