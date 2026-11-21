//! Less of a song: a stretch of it, some of its tracks, or both.
//!
//! An excerpt is **not a new sound**. It is the same renderer asked for less
//! of the same piece, and the promise it makes is exact: the samples a window
//! hands back are the samples a whole render would have put there, bit for
//! bit. That promise is the whole feature — a mix decision taken on eight bars
//! is only worth taking if those eight bars are the ones the file will have.
//!
//! ## Which clock a window counts on
//!
//! **The rendered piece, not the written arrangement.** `fit` can change both
//! the tempo and the number of passes, and `automation` already reads its
//! beats off the result rather than off the page — so a window that counted
//! the arrangement would be the one coordinate in the crate that disagreed
//! with the others. Under a `loop` fit the two genuinely differ: beat 40 of a
//! sixteen-beat arrangement looped four times is in the third pass, and that
//! is what a window asking for beat 40 gets, because it is what is audible
//! there.
//!
//! Seconds count on the same clock, and are the same window said the other
//! way: at the tempo the piece is actually rendered at.
//!
//! ## Why a window still renders what comes before it
//!
//! Everything in the signal path is causal but for two stages, and both of
//! them are the reason a window cannot simply start where it starts:
//!
//! - a note before the window rings into it, and its bus and the song's chain
//!   decay across the seam;
//! - the limiter and any compressor **look ahead**, so a peak just past the
//!   window pulls the gain down inside it.
//!
//! So a window renders the piece from the top and keeps the stretch asked
//! for, and it renders a [`guard`] past the end for the lookahead to see.
//! What it saves is every note after that guard, which for the case this
//! exists to serve — the first bars of a long bed — is nearly all of them.
//! A window at the very end of a piece saves nothing, and that is honest
//! rather than a gap: you cannot know what the last bar sounds like without
//! playing up to it.
//!
//! **A song with a `wrap` tail saves nothing either**, wherever the window
//! is: its first bars carry the ring-out of its last, so the last notes reach
//! everything. The window still comes back exact, because it is still the
//! whole render cut down; it just costs what the whole render costs.

mod scope;
mod span;

use std::fmt;

use super::Song;
use super::clock::Clock;
use crate::core::RATE;

pub(crate) use scope::Scope;
pub use span::{Span, SpanError};

/// What less of a song to render: a stretch of it, a set of its tracks, or
/// both. The default is the whole piece, which is what every render that does
/// not ask for less passes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Excerpt {
    /// Which stretch of the rendered piece to keep. `None` is all of it.
    pub window: Option<Window>,
    /// Which tracks reach the mix, by name. Empty is all of them.
    ///
    /// A track left out is still **played** when something is keyed off it, so
    /// a solo shows the duck the mix has rather than one it does not. It is
    /// simply not heard.
    pub only: Vec<String>,
}

impl Excerpt {
    /// True when this asks for the piece exactly as a whole render makes it.
    pub fn is_whole(&self) -> bool {
        self.window.is_none() && self.only.is_empty()
    }

    /// A stretch of the piece, and every track in it.
    pub fn of(window: Window) -> Self {
        Self {
            window: Some(window),
            only: Vec::new(),
        }
    }

    /// These tracks, across the whole piece.
    pub fn only(tracks: Vec<String>) -> Self {
        Self {
            window: None,
            only: tracks,
        }
    }
}

/// A stretch of the rendered piece, counted in beats or in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    span: Span,
    unit: Unit,
}

/// What a window's two numbers count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Beats of the rendered piece — the unit the document itself is written
    /// in, so a window lines up with a pattern length and an automation point
    /// without anybody converting anything.
    Beats,
    /// Seconds of the rendered piece, for a question the picture asked rather
    /// than the music.
    Seconds,
}

impl Window {
    /// A window in beats of the rendered piece.
    pub const fn beats(span: Span) -> Self {
        Self {
            span,
            unit: Unit::Beats,
        }
    }

    /// A window in seconds of the rendered piece.
    pub const fn seconds(span: Span) -> Self {
        Self {
            span,
            unit: Unit::Seconds,
        }
    }

    /// The first sample-frame kept, and the one just past the last — `None`
    /// meaning however long the piece turns out to be.
    ///
    /// `clock` is the tempo the piece is **rendered** at, which under a
    /// `stretch` fit is not the one written down — and the same conversion
    /// the notes were placed with, so a window in beats opens on the beat.
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

/// What was asked for, in the words it was asked in — so a report of an
/// excerpt says what it is a report *of*.
///
/// A number read off a partial bake means nothing without that: "mean −15 dBFS"
/// is a different finding over eight bars than over the whole piece, and the
/// line carrying it is the only place a reader can be told which.
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
