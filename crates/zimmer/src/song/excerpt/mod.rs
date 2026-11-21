//! Less of a song: a stretch of it, some of its tracks, or both.

mod scope;
mod span;

use std::fmt;

use super::Song;
use super::clock::Clock;
use crate::core::RATE;

pub(crate) use scope::Scope;
pub use span::{Span, SpanError};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Excerpt {
    pub window: Option<Window>,
    pub only: Vec<String>,
}

impl Excerpt {
    pub fn is_whole(&self) -> bool {
        self.window.is_none() && self.only.is_empty()
    }

    pub fn of(window: Window) -> Self {
        Self {
            window: Some(window),
            only: Vec::new(),
        }
    }

    pub fn only(tracks: Vec<String>) -> Self {
        Self {
            window: None,
            only: tracks,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    span: Span,
    unit: Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Beats,
    Seconds,
}

impl Window {
    pub const fn beats(span: Span) -> Self {
        Self {
            span,
            unit: Unit::Beats,
        }
    }

    pub const fn seconds(span: Span) -> Self {
        Self {
            span,
            unit: Unit::Seconds,
        }
    }

    fn frames(self, clock: &Clock) -> (usize, Option<usize>) {
        let seconds = |value: f32| match self.unit {
            Unit::Beats => clock.seconds(value),
            Unit::Seconds => value,
        };
        let frame = |value: f32| (seconds(value) * RATE).round().max(0.0) as usize;
        (frame(self.span.start), self.span.end.map(frame))
    }
}

impl fmt::Display for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = match self.unit {
            Unit::Beats => "beats",
            Unit::Seconds => "seconds",
        };
        write!(f, "{unit} {}", self.span)
    }
}

impl fmt::Display for Excerpt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut said = Vec::new();
        if let Some(window) = self.window {
            said.push(window.to_string());
        }
        if !self.only.is_empty() {
            said.push(format!("only {}", self.only.join(" + ")));
        }
        if said.is_empty() {
            return f.write_str("all of it");
        }
        f.write_str(&said.join(", "))
    }
}
