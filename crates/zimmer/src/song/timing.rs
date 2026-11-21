//! Making a song the length the picture needs.

use serde::{Deserialize, Serialize};

use super::Song;
use crate::error::SynthError;

pub(crate) const MAX_STRETCH: f32 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<FitTo>,
    #[serde(default)]
    pub mode: FitMode,
}

impl Fit {
    pub fn lasting(seconds: f32, mode: FitMode) -> Self {
        Self {
            seconds: Some(seconds),
            to: None,
            mode,
        }
    }

    pub fn resolved(self, seconds: f32) -> Self {
        Self::lasting(seconds, self.mode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitTo {
    Clip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    #[default]
    Loop,
    Stretch,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    #[serde(default)]
    pub in_seconds: f32,
    #[serde(default)]
    pub out_seconds: f32,
}

impl Fade {
    pub(crate) fn is_silent_about_everything(self) -> bool {
        self.in_seconds <= 0.0 && self.out_seconds <= 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tail {
    #[default]
    Ring,
    Exact,
    Wrap,
}

pub(crate) fn check_wrap(fit: Option<Fit>, fade: Option<Fade>) -> Result<(), SynthError> {
    if fade.is_some_and(|fade| !fade.is_silent_about_everything()) {
        return Err(SynthError::WrapWith {
            field: "`fade`",
            why: "on a loop it is a dip in the level every time round — fade the music in \
                  or out where it is played instead",
        });
    }
    let (field, why) = match fit.map(|fit| fit.mode) {
        Some(FitMode::Loop) => (
            "`fit` in mode `loop` (the default)",
            "it cuts mid-pass, so the file has no single point where the music comes back \
             round — use mode `stretch`",
        ),
        Some(FitMode::Once) => (
            "`fit` in mode `once`",
            "it pads with silence, which the loop would play every time round — use mode \
             `stretch`",
        ),
        Some(FitMode::Stretch) | None => return Ok(()),
    };
    Err(SynthError::WrapWith { field, why })
}

pub(super) fn check(song: &Song) -> Result<(), SynthError> {
    if let Some(fade) = song.fade.filter(|fade| {
        !(fade.in_seconds.is_finite()
            && fade.in_seconds >= 0.0
            && fade.out_seconds.is_finite()
            && fade.out_seconds >= 0.0)
    }) {
        return Err(SynthError::BadFade {
            seconds: fade.in_seconds.max(fade.out_seconds),
        });
    }
    if song.tail() == Tail::Wrap {
        check_wrap(song.fit, song.fade)?;
    }
    let Some(fit) = song.fit else {
        return Ok(());
    };
    let seconds = match (fit.seconds, fit.to) {
        (Some(seconds), None) => seconds,
        (None, None) => {
            return Err(SynthError::FitLength {
                why: "needs a length: `seconds`, or `to: \"clip\"` for as long as the \
                      clip that plays the song",
            });
        }
        (Some(_), Some(_)) => {
            return Err(SynthError::FitLength {
                why: "takes `seconds` or `to`, not both — `to` says where the length \
                      comes from, so a number beside it would be ignored",
            });
        }
        (None, Some(_)) => {
            return Err(SynthError::FitLength {
                why: "is `to: \"clip\"`, and nothing has said how long that clip is — \
                      it is resolved by whatever bakes the song from a project, which \
                      reads the clip; rendering the recipe alone needs `seconds`",
            });
        }
    };
    if !(seconds.is_finite() && seconds > 0.0) {
        return Err(SynthError::BadFitSeconds { seconds });
    }
    let stretch = song
        .anchors
        .is_empty()
        .then(|| super::shape::stretch_ratio(song, fit, seconds))
        .flatten();
    if let Some(ratio) = stretch.filter(|ratio| ratio.abs() > MAX_STRETCH) {
        return Err(SynthError::StretchTooFar {
            bpm: song.bpm,
            needed: song.bpm * (1.0 + ratio),
            limit: MAX_STRETCH,
        });
    }
    Ok(())
}
