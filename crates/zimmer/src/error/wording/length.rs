//! What a note's length, and a song's length and level fields, are refused with: `duration`, a note
//! too long to render, `fit`, `fade`, `tail`, a song too long to render and `anchors`.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_length(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadDuration { duration, .. } => write!(
                f,
                "note: `duration` must be positive seconds, got {duration}"
            ),
            Self::NoteTooLong {
                at,
                held,
                tail,
                limit,
            } => {
                match at {
                    Some(at) => write!(
                        f,
                        "song: the note on track `{}` at beat {} of pattern `{}`",
                        at.track, at.start, at.pattern
                    )?,
                    None => write!(f, "note:")?,
                }
                write!(
                    f,
                    " sounds {:.2} s — held {held:.2} s, ringing {tail:.2} s more — and a note sounds at most {limit} s: shorten it, its release or its effect tail",
                    held + tail
                )
            }
            Self::BadFitSeconds { seconds, .. } => {
                write!(f, "song: `fit.seconds` must be positive, got {seconds}")
            }
            Self::FitLength { why, .. } => write!(f, "song: `fit` {why}"),
            Self::BadAnchor { section, why, .. } => {
                write!(f, "song: the anchor on section {section} {why}")
            }
            Self::BadFade { seconds, .. } => write!(
                f,
                "song: a fade must be zero or more seconds, got {seconds}"
            ),
            Self::StretchTooFar {
                bpm, needed, limit, ..
            } => write!(
                f,
                "song: fitting this at `stretch` needs {needed:.1} bpm against {bpm:.1} written, further than the {}% a piece survives — use `loop`, or change the arrangement",
                (limit * 100.0).round()
            ),
            Self::WrapWith { field, why, .. } => {
                write!(f, "song: `tail: wrap` cannot take {field}: {why}")
            }
            Self::WrapOverhang {
                overhang, length, ..
            } => write!(
                f,
                "song: `tail: wrap` would fold {overhang:.2} s of ring-out onto a {length:.2} s loop, and it would still be ringing the next time round — shorten the release or the effect tail, or lengthen the arrangement"
            ),
            Self::SongTooLong { seconds, limit } => write!(
                f,
                "song: it would render {seconds:.0} s, and a song renders at most {limit} s (ten minutes) — shorten its arrangement or its `fit`, or bake it as two songs"
            ),
            other => write!(f, "{other:?}"),
        }
    }
}
