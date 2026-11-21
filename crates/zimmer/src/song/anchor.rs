//! Sections that land on the cut: a song's sections pinned to times.

use serde::{Deserialize, Serialize};

use super::Song;
use super::clock::Clock;
use super::timing::{FitMode, MAX_STRETCH};
use crate::error::SynthError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchor {
    pub section: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
}

impl Anchor {
    pub fn at(section: usize, seconds: f32) -> Self {
        Self {
            section,
            seconds: Some(seconds),
            clip: None,
        }
    }

    pub fn resolved(&self, seconds: f32) -> Self {
        Self::at(self.section, seconds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Landing {
    pub(crate) beat: f64,
    pub(crate) seconds: f64,
    pub(crate) section: usize,
}

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
