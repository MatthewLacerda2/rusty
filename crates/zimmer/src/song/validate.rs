//! What a song is refused for, before any samples are produced.

use super::automate::{Automation, Param};
use super::chord::Led;
use super::{ArrangementEntry, Context, Humanize, Key, Layer, Pattern, PatternEntry, Song, Track};
use crate::error::SynthError;
use crate::patch::Patch;

impl Song {
    pub fn validate(&self) -> Result<(), SynthError> {
        if !(self.bpm.is_finite() && self.bpm > 0.0) {
            return Err(SynthError::BadBpm { bpm: self.bpm });
        }
        super::tempo::check(&self.tempo)?;
        if self.tracks.is_empty() {
            return Err(SynthError::NoTracks);
        }
        if self.arrangement.is_empty() {
            return Err(SynthError::EmptyArrangement);
        }
        let key = self.key()?;
        for entry in &self.arrangement {
            if matches!(entry, ArrangementEntry::Layered(layered) if layered.layers.is_empty()) {
                return Err(SynthError::NoLayers);
            }
            for layer in entry.layers() {
                if !self.patterns.contains_key(layer.pattern()) {
                    return Err(SynthError::UnknownPattern {
                        pattern: layer.pattern().to_owned(),
                    });
                }
                self.check_layer(layer, key.as_ref())?;
            }
        }
        for (name, pattern) in &self.patterns {
            pattern.validate(name, &self.tracks, key.as_ref())?;
        }
        crate::patch::check_chain(&self.fx)?;
        for track in &self.tracks {
            crate::patch::check_chain(&track.fx)?;
        }
        self.check_sidechains()?;
        self.check_automation()?;
        self.check_feel()?;
        super::timing::check(self)?;
        super::anchor::check(self)?;
        super::cap::check(self, key.as_ref())
    }

    fn check_sidechains(&self) -> Result<(), SynthError> {
        crate::patch::check_no_sidechain(&self.fx, "song")?;
        for track in &self.tracks {
            for key in crate::patch::sidechains(&track.fx) {
                if key == track.name {
                    return Err(SynthError::SelfSidechain {
                        track: track.name.clone(),
                    });
                }
                if !self.tracks.iter().any(|it| it.name == key) {
                    return Err(SynthError::UnknownSidechain {
                        track: track.name.clone(),
                        key: key.to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    fn check_automation(&self) -> Result<(), SynthError> {
        for (index, curve) in self.automation.iter().enumerate() {
            let (track, param) = (curve.track.clone(), curve.param.as_str());
            if !self.tracks.iter().any(|it| it.name == curve.track) {
                return Err(SynthError::UnknownAutomationTrack { track, param });
            }
            if self.automation[..index]
                .iter()
                .any(|earlier| earlier.track == curve.track && earlier.param == curve.param)
            {
                return Err(SynthError::DuplicateAutomation { track, param });
            }
            check_curve(curve)?;
            if curve.param == Param::Send && !super::render::has_a_room(&self.fx) {
                return Err(SynthError::AutomationWithoutRoom { track });
            }
        }
        Ok(())
    }

    fn check_feel(&self) -> Result<(), SynthError> {
        if !(self.swing.is_finite() && (0.0..1.0).contains(&self.swing)) {
            return Err(SynthError::BadSwing { swing: self.swing });
        }
        if let Some((at, amount)) = self.humanize.and_then(unscattered) {
            let field = ["timing", "velocity", "timbre"][at];
            return Err(SynthError::BadHumanize { field, amount });
        }
        for track in &self.tracks {
            let refuse = |field, amount| SynthError::BadTrackFeel {
                track: track.name.clone(),
                field,
                amount,
            };
            if !track.push.is_finite() {
                return Err(refuse("push", track.push));
            }
            if let Some(swing) = track
                .swing
                .filter(|swing| !(swing.is_finite() && (0.0..1.0).contains(swing)))
            {
                return Err(refuse("swing", swing));
            }
            if let Some((at, amount)) = track.humanize.and_then(unscattered) {
                let field = ["humanize.timing", "humanize.velocity", "humanize.timbre"][at];
                return Err(refuse(field, amount));
            }
        }
        Ok(())
    }

    fn check_layer(&self, layer: &Layer, key: Option<&Key>) -> Result<(), SynthError> {
        let Layer::Transformed(play) = layer else {
            return Ok(());
        };
        if let Some(transpose) = play.transpose.filter(|transpose| !transpose.is_finite()) {
            return Err(SynthError::BadTranspose {
                pattern: play.pattern.clone(),
                transpose,
            });
        }
        if play.transpose_degrees.is_some() {
            if play.transpose.is_some() {
                return Err(SynthError::TwoTransposes {
                    pattern: play.pattern.clone(),
                });
            }
            if key.is_none() {
                return Err(SynthError::DiatonicWithoutKey {
                    pattern: play.pattern.clone(),
                });
            }
        }
        if let Some(scale) = play
            .vel_scale
            .filter(|scale| !(scale.is_finite() && *scale >= 0.0))
        {
            return Err(SynthError::BadVelocityScale {
                pattern: play.pattern.clone(),
                scale,
            });
        }
        for wanted in play.tracks.iter().flatten() {
            if !self.tracks.iter().any(|track| &track.name == wanted) {
                return Err(SynthError::UnknownTrackFilter {
                    pattern: play.pattern.clone(),
                    track: wanted.clone(),
                });
            }
        }
        Ok(())
    }
}

impl Pattern {
    fn validate(&self, name: &str, tracks: &[Track], key: Option<&Key>) -> Result<(), SynthError> {
        if !(self.beats.is_finite() && self.beats > 0.0) {
            return Err(SynthError::BadPatternBeats {
                pattern: name.to_owned(),
                beats: self.beats,
            });
        }
        let around = Context {
            beats: self.beats,
            key,
        };
        let mut led = Led::default();
        for (index, entry) in self.notes.iter().enumerate() {
            entry.validate(name, index, tracks, around, &mut led)?;
        }
        Ok(())
    }
}

impl PatternEntry {
    fn validate(
        &self,
        pattern: &str,
        index: usize,
        tracks: &[Track],
        around: Context<'_>,
        led: &mut Led,
    ) -> Result<(), SynthError> {
        let pattern = || pattern.to_owned();
        if !tracks.iter().any(|track| track.name == self.track()) {
            return Err(SynthError::UnknownTrack {
                pattern: pattern(),
                index,
                track: self.track().to_owned(),
            });
        }
        if !(self.start().is_finite() && self.start() >= 0.0) {
            return Err(SynthError::BadNoteStart {
                pattern: pattern(),
                index,
                start: self.start(),
            });
        }
        self.voice_into(around, led, &mut Vec::new())?;
        if !(self.dur().is_finite() && self.dur() > 0.0) {
            return Err(SynthError::BadNoteDuration {
                pattern: pattern(),
                index,
                dur: self.dur(),
            });
        }
        Ok(())
    }
}

fn unscattered(feel: Humanize) -> Option<(usize, f32)> {
    [feel.timing, feel.velocity, feel.timbre]
        .into_iter()
        .enumerate()
        .find(|(_, amount)| !(amount.is_finite() && *amount >= 0.0))
}

fn check_curve(curve: &Automation) -> Result<(), SynthError> {
    let (track, param) = (curve.track.clone(), curve.param.as_str());
    let bad_curve = |why| SynthError::BadAutomationCurve {
        track: curve.track.clone(),
        param,
        why,
    };
    if curve.points.is_empty() {
        return Err(bad_curve("it has no points, so it moves nothing"));
    }
    for point in &curve.points {
        let bad_point = |field, value| SynthError::BadAutomationPoint {
            track: curve.track.clone(),
            param,
            field,
            value,
        };
        if !(point.beat.is_finite() && point.beat >= 0.0) {
            return Err(bad_point("beat", point.beat));
        }
        if !point.value.is_finite() {
            return Err(bad_point("value", point.value));
        }
        if curve.param == Param::Cutoff && point.value <= 0.0 {
            return Err(SynthError::BadAutomationCutoff {
                track,
                cutoff: point.value,
            });
        }
    }
    // Last, so a NaN beat is reported as the number it is not rather than as an ordering it happens
    // to break.
    if !curve.is_sorted() {
        return Err(bad_curve("`beat` must ascend, and no two may be equal"));
    }
    Ok(())
}

pub(super) fn check_resolved(song: &Song, patches: &[Patch]) -> Result<(), SynthError> {
    for curve in &song.automation {
        if curve.param != Param::Cutoff {
            continue;
        }
        let filtered = song
            .tracks
            .iter()
            .zip(patches)
            .find(|(track, _)| track.name == curve.track)
            .is_some_and(|(_, patch)| patch.filter.is_some());
        if !filtered {
            return Err(SynthError::AutomationWithoutFilter {
                track: curve.track.clone(),
            });
        }
    }
    Ok(())
}
