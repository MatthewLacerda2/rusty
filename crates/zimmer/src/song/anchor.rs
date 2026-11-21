//! Sections that land on the cut: a song's sections pinned to times.
//!
//! [`Fit`](super::Fit) keeps a song's *length* honest when the cut moves. It
//! does not move the *moments*: the impact on the logo, the lift when the deal
//! is won, the last hit on the last word. Those are where a section starts, and
//! a cut shortened by ten seconds leaves every one of them off its bar line
//! (#1009). An anchor says where one must fall:
//!
//! ```jsonc
//! "anchors": [
//!   { "section": 3, "seconds": 12.4 },   // the reveal starts at 12.4 s
//!   { "section": 6, "clip": "end-card" } // the CTA where that clip starts
//! ]
//! ```
//!
//! **Each stretch between two anchors gets one tempo**, chosen so the second
//! anchor's downbeat lands on its time: the written tempo, scaled by one factor
//! over that stretch — a tempo map with a step at each anchored downbeat, where
//! a crash or a change of groove hides it. A song that writes its own
//! [`tempo`](super::Song::tempo) map keeps its shape inside each stretch, the
//! way a `stretch` fit keeps it over the whole piece. Sections after the last
//! anchor keep the written tempo, unless a `stretch` fit lands the end of the
//! arrangement on its length — which is one more anchor, and is treated as one.
//!
//! A `section` is an arrangement entry, counted from 0 — the same rows
//! [`Song::sections`](super::Song::sections) reports. Written in order, one
//! anchor per section at most, each later than the one before.
//!
//! **This crate never learns what a clip is.** `clip` is a name for whoever
//! bakes the song from a project to resolve into `seconds`
//! ([`Anchor::resolved`]); a song still carrying one is refused rather than
//! rendered with the section wherever it happened to fall.

use serde::{Deserialize, Serialize};

use super::Song;
use super::clock::Clock;
use super::timing::{FitMode, MAX_STRETCH};
use crate::error::SynthError;

/// One section pinned to a time in the piece.
///
/// Written one of two ways, and exactly one: at a number of `seconds`, or at a
/// `clip` — the start of a clip, for a caller that can read one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    /// The arrangement entry whose first downbeat is pinned, counted from 0.
    pub section: usize,
    /// Seconds into the piece that downbeat must fall at. Absent when `clip`
    /// says where it comes from instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
    /// The clip whose start the downbeat lands on — a name only the caller can
    /// read, resolved into `seconds` before rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
}

impl Anchor {
    /// Section `section` pinned at `seconds` into the piece.
    pub fn at(section: usize, seconds: f32) -> Self {
        Self {
            section,
            seconds: Some(seconds),
            clip: None,
        }
    }

    /// This anchor with its time decided: `seconds`, and no `clip` left to
    /// resolve.
    pub fn resolved(&self, seconds: f32) -> Self {
        Self::at(self.section, seconds)
    }
}

/// One downbeat the planned clock must put at a time: an anchor, or the end
/// of a `stretch` fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Landing {
    /// The beat of one pass it falls on.
    pub(crate) beat: f64,
    /// Seconds into the piece it must fall at.
    pub(crate) seconds: f64,
    /// The section that starts there — the arrangement's length for its end.
    pub(crate) section: usize,
}

/// The downbeats this song's clock must land, in order, or `None` for a song
/// that pins nothing — which keeps the clock it always had.
///
/// An anchor on the first section is dropped: it starts the piece, at zero,
/// which every clock already does. An anchor not yet resolved, or pointing
/// past the arrangement, makes the whole answer `None` — `validate` refuses
/// both, and a question asked of the document without rendering it is
/// answered with the song as written.
pub(crate) fn landings(song: &Song) -> Option<Vec<Landing>> {
    if song.anchors.is_empty() {
        return None;
    }
    let starts = starts(song);
    let mut landings = Vec::with_capacity(song.anchors.len() + 1);
    for anchor in &song.anchors {
        let beat = *starts.get(anchor.section)?;
        let seconds = anchor.seconds?;
        if beat > 0.0 {
            landings.push(Landing {
                beat: f64::from(beat),
                seconds: f64::from(seconds),
                section: anchor.section,
            });
        }
    }
    if let Some(fit) = song.fit
        && fit.mode == FitMode::Stretch
        && let Some(seconds) = fit.seconds
    {
        landings.push(Landing {
            beat: f64::from(song.arrangement_beats()),
            seconds: f64::from(seconds),
            section: song.arrangement.len(),
        });
    }
    (!landings.is_empty()).then_some(landings)
}

/// The beat each arrangement entry starts on, in one pass.
fn starts(song: &Song) -> Vec<f32> {
    let mut beat = 0.0;
    song.arrangement
        .iter()
        .map(|entry| {
            let start = beat;
            beat += song.slot_beats(entry).unwrap_or(0.0);
            start
        })
        .collect()
}

/// Refuses anchors the clock cannot honour, before anything is rendered.
///
/// One that names no section, writes both or neither of its fields, carries a
/// `clip` nobody resolved, or is not a time; anchors out of order, in sections
/// or in seconds — which would have the music play a stretch backwards; one on
/// the first section anywhere but zero; any beside a `loop` fit, whose later
/// passes have no times to land on; and, like a `stretch` fit, a stretch whose
/// tempo would move more than a quarter from the one written.
pub(super) fn check(song: &Song) -> Result<(), SynthError> {
    let Some(first) = song.anchors.first() else {
        return Ok(());
    };
    if song.fit.is_some_and(|fit| fit.mode == FitMode::Loop) {
        return Err(refuse(
            first.section,
            "cannot be kept under `fit` in mode `loop`, which plays the arrangement again \
             where those times mean nothing — use `stretch` or `once`"
                .into(),
        ));
    }
    let starts = starts(song);
    let mut previous: Option<(usize, f32)> = None;
    for anchor in &song.anchors {
        let seconds = written(anchor, starts.len())?;
        if starts[anchor.section] == 0.0 && seconds != 0.0 {
            let why = format!("starts the song, so it is at 0 s, not {seconds}");
            return Err(refuse(anchor.section, why));
        }
        if let Some((section, at)) = previous {
            if anchor.section <= section {
                let why = format!("comes after section {section}, so it must be a later one");
                return Err(refuse(anchor.section, why));
            }
            if seconds <= at {
                let why =
                    format!("lands at {seconds:.3} s, not after section {section} at {at:.3} s");
                return Err(refuse(anchor.section, why));
            }
        }
        previous = Some((anchor.section, seconds));
    }
    if let (Some(fit), Some((section, at))) = (song.fit, previous)
        && fit.mode == FitMode::Stretch
        && fit.seconds.is_some_and(|end| end <= at)
    {
        let why = format!("lands at {at:.3} s, not before the end `fit` stretches to");
        return Err(refuse(section, why));
    }
    check_tempo(song)
}

/// The time an anchor is written at, once its fields have been checked.
fn written(anchor: &Anchor, sections: usize) -> Result<f32, SynthError> {
    if anchor.section >= sections {
        let why = format!("is past the arrangement, which has {sections} (counted from 0)");
        return Err(refuse(anchor.section, why));
    }
    let seconds = match (anchor.seconds, &anchor.clip) {
        (Some(seconds), None) => seconds,
        (None, None) => {
            let why = "needs `seconds`, or `clip` for where a clip starts".into();
            return Err(refuse(anchor.section, why));
        }
        (Some(_), Some(_)) => {
            let why = "takes `seconds` or `clip`, not both".into();
            return Err(refuse(anchor.section, why));
        }
        (None, Some(clip)) => {
            let why = format!(
                "is at clip `{clip}`, and nothing has said where that clip starts — whatever \
                 bakes the song from a project resolves it; rendering the recipe alone needs \
                 `seconds`"
            );
            return Err(refuse(anchor.section, why));
        }
    };
    if !(seconds.is_finite() && seconds >= 0.0) {
        let why = format!("must be zero or more seconds, got {seconds}");
        return Err(refuse(anchor.section, why));
    }
    Ok(seconds)
}

/// The bound a `stretch` fit has, applied to each stretch between landings:
/// past it the music is not this music any more, and the refusal says what
/// tempo it would have needed.
fn check_tempo(song: &Song) -> Result<(), SynthError> {
    let Some(landings) = landings(song) else {
        return Ok(());
    };
    let clock = Clock::written(song, 1);
    let (mut beat, mut seconds) = (0.0, 0.0);
    for landing in landings {
        let wrote = f64::from(clock.seconds(landing.beat as f32) - clock.seconds(beat as f32));
        let factor = wrote / (landing.seconds - seconds);
        if (factor - 1.0).abs() > f64::from(MAX_STRETCH) {
            let bpm = f64::from(clock.tempo(beat as f32));
            let end = if landing.section == song.arrangement.len() {
                "is the end `fit` stretches to, and "
            } else {
                ""
            };
            let why = format!(
                "{end}needs {:.1} bpm from beat {beat} to land at {:.3} s, against {bpm:.1} bpm written — \
                 further than the {}% a piece survives; move it, or change the arrangement",
                bpm * factor,
                landing.seconds,
                (MAX_STRETCH * 100.0).round()
            );
            return Err(refuse(landing.section, why));
        }
        (beat, seconds) = (landing.beat, landing.seconds);
    }
    Ok(())
}

fn refuse(section: usize, why: String) -> SynthError {
    SynthError::BadAnchor { section, why }
}
