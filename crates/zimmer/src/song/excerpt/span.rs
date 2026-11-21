//! The grammar of "part of it": `12:48`, `12:`, `:48`.
//!
//! Its own file because it is a *notation* rather than a decision — the same
//! two numbers and one colon that `render --range` already takes, and the only
//! thing here that a caller types by hand. What those numbers then mean is
//! [`super::Window`]'s business, and keeping the two apart is what lets the
//! same span be read as beats or as seconds without the parser knowing which.

use std::fmt;
use std::str::FromStr;

/// Two numbers with a colon between them: `12:48` is 12 up to but not
/// including 48, `12:` runs to the end of the piece, `:48` from its start.
///
/// End-exclusive, and spelled the way `render --range` already spells a slice
/// of the timeline: one grammar for "part of it" across the program is one
/// fewer thing to look up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// Where it opens. Never negative and always known — an open start is
    /// zero.
    pub(super) start: f32,
    /// Where it closes, or `None` to run to the end of whatever it is a span
    /// of. Only [`super::Window`] knows what that is.
    pub(super) end: Option<f32>,
}

impl Span {
    /// A span from `start`, ending at `end` or running to the end of the
    /// piece.
    pub fn new(start: f32, end: Option<f32>) -> Result<Self, SpanError> {
        if !start.is_finite() || start < 0.0 {
            return Err(SpanError::Malformed);
        }
        match end {
            Some(end) if !end.is_finite() || end <= start => Err(SpanError::Backwards),
            _ => Ok(Self { start, end }),
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.end {
            Some(end) => write!(f, "{}:{end}", self.start),
            None => write!(f, "{}:", self.start),
        }
    }
}

impl FromStr for Span {
    type Err = SpanError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (start, end) = text.split_once(':').ok_or(SpanError::Malformed)?;
        let part = |text: &str| -> Result<Option<f32>, SpanError> {
            let text = text.trim();
            if text.is_empty() {
                return Ok(None);
            }
            text.parse::<f32>()
                .map(Some)
                .map_err(|_| SpanError::Malformed)
        };
        Self::new(part(start)?.unwrap_or(0.0), part(end)?)
    }
}

/// Text that is not a span.
///
/// Written out by hand rather than derived, because this crate has no error
/// derive to reach for — [`SynthError`](crate::SynthError) says its own
/// wording in one place too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanError {
    /// No colon, a number that is not one, or a negative start.
    Malformed,
    /// Includes the equal case: end-exclusive means `8:8` selects nothing.
    Backwards,
}

impl fmt::Display for SpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => f.write_str("expected a range like `0:32`, `16:`, or `:32`"),
            Self::Backwards => f.write_str("a range has to end after it begins"),
        }
    }
}

impl std::error::Error for SpanError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_reads_the_three_ways_it_is_written() {
        assert_eq!(
            "0:32".parse::<Span>(),
            Ok(Span::new(0.0, Some(32.0)).unwrap())
        );
        assert_eq!("16:".parse::<Span>(), Ok(Span::new(16.0, None).unwrap()));
        assert_eq!(
            ":32".parse::<Span>(),
            Ok(Span::new(0.0, Some(32.0)).unwrap())
        );
    }

    #[test]
    fn a_span_that_selects_nothing_is_refused() {
        assert_eq!("8:8".parse::<Span>(), Err(SpanError::Backwards));
        assert_eq!("8:4".parse::<Span>(), Err(SpanError::Backwards));
        assert_eq!("8".parse::<Span>(), Err(SpanError::Malformed));
        assert_eq!("-1:8".parse::<Span>(), Err(SpanError::Malformed));
    }

    /// The wording of a refusal is the whole of what a caller repairing a
    /// command line reads, so it is asserted rather than assumed.
    #[test]
    fn a_refusal_says_what_a_range_should_look_like() {
        assert!(
            SpanError::Malformed.to_string().contains("0:32"),
            "got {}",
            SpanError::Malformed
        );
        assert!(
            SpanError::Backwards
                .to_string()
                .contains("end after it begins"),
            "got {}",
            SpanError::Backwards
        );
    }
}
