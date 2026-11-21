//! What a song is refused for, before any samples are produced.
//!
//! A typo'd track or pattern name would otherwise be *silence in the middle of
//! a piece*, which is the failure mode that costs an agent a whole iteration to
//! even notice. So every name is resolved and every number is checked up front.

use super::automate::{Automation, Param};
use super::chord::Led;
use super::{ArrangementEntry, Context, Humanize, Key, Layer, Pattern, PatternEntry, Song, Track};
use crate::error::SynthError;
use crate::patch::Patch;

impl Song {
    /// Rejects a song the renderer cannot honour, *before* any samples are
    /// produced — so a typo'd track or pattern name is a clear message rather
    /// than silence in the mix, which is the failure mode that would cost an
    /// agent a whole iteration to even notice.
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
        // The key first: a degree and a diatonic lift are both refused for
        // lack of one, so "there is no key" has to be told from "the key does
        // not parse" before either is reported.
        let key = self.key()?;
        for entry in &self.arrangement {
            if let ArrangementEntry::Layered(layered) = entry
                && layered.layers.is_empty()
            {
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
        // The two chains the song owns. A track's inline patch carries a third,
        // and it is checked where every other patch field is — at the moment a
        // note of it is rendered.
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

    /// Who is keyed from whom.
    ///
    /// A sidechain names a track, and a track name means something in exactly
    /// one of the three places a chain can live — so the song's own chain
    /// refuses one outright, and a track's has to name a real track that is
    /// not itself. Both are the typo that would otherwise be *a duck that
    /// never happens*, which is the same failure mode as a typo'd track name
    /// and gets the same treatment.
    ///
    /// What is deliberately **not** here is a cycle check. The part a key
    /// hands over is the instrument as played, before any chain runs, so two
    /// tracks pressing each other down is well defined rather than circular —
    /// `super::mix` has the reasoning.
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

    /// Every curve that moves a value across the piece.
    ///
    /// A curve is refused for everything that would leave it moving *nothing*,
    /// which is the same failure as a typo'd track name and gets the same
    /// treatment: an automation nobody can hear is worse than one that is
    /// refused, because the recipe still says the build is there.
    ///
    /// So: a track that does not exist, no points at all, two curves on one
    /// track and parameter, beats that do not ascend, and a beat or a value
    /// that is not a number. A `cutoff` point at or below zero is refused for
    /// the reason a written cutoff is — there is no filter at 0 Hz. A `send`
    /// curve is refused on a song whose chain has no `reverb` or `delay`,
    /// since a send reaches nothing else.
    ///
    /// What is **not** here is the other half of the same idea: a `cutoff`
    /// curve on a track whose instrument has no filter. That one needs the
    /// patch, which a named reference does not carry, so it is checked in
    /// [`check_resolved`] the moment the resolver has answered.
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
            if curve.param == Param::Send && !super::mix::has_a_room(&self.fx) {
                return Err(SynthError::AutomationWithoutRoom { track });
            }
        }
        Ok(())
    }

    /// The performance fields: how much the song swings, and how far its player
    /// strays from the page — and the same of each track that writes its own,
    /// with how far it sits off the beat.
    ///
    /// Both are refused only for values that stop meaning what the field is
    /// named after. A swing at or past 1 does not swing harder — it moves the
    /// off-beat onto the following downbeat and reorders the music — and a
    /// negative scatter is not a scatter in the other direction. Taste is not
    /// checked: a song that swings 0.9 renders, and sounds like it.
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
            if let Some(swing) = track.swing
                && !(swing.is_finite() && (0.0..1.0).contains(&swing))
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

    /// One layer's transforms — a whole entry's, unless it is layered.
    ///
    /// A `tracks` filter naming no real track is the same typo as an
    /// arrangement naming no real pattern, and has the same consequence — an
    /// instrument that silently never plays — so it gets the same treatment.
    /// A transpose or a scale that is not a finite number is refused here
    /// too: a NaN reaches the mix as a whole song of silence, which is a long
    /// way from the field that caused it.
    ///
    /// The diatonic transpose is refused for two further reasons, both of them
    /// decisions rather than arithmetic — see
    /// [`transpose_degrees`](super::Play::transpose_degrees).
    fn check_layer(&self, layer: &Layer, key: Option<&Key>) -> Result<(), SynthError> {
        let Layer::Transformed(play) = layer else {
            return Ok(());
        };
        if let Some(transpose) = play.transpose
            && !transpose.is_finite()
        {
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
        if let Some(scale) = play.vel_scale
            && !(scale.is_finite() && scale >= 0.0)
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
    /// Checks one pattern's slot length and every note in it.
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
    /// Checks that one entry names a real track, starts somewhere, lasts for
    /// some time, and has pitches that resolve.
    ///
    /// Every entry that is not already a pitch resolves by being expanded,
    /// which is the same work the renderer does and therefore the same answer:
    /// a chord name off the table, a voicing pushed off the keyboard, a step
    /// string that does not fill its pattern, a degree that counts from zero
    /// or is written where the song declares no key — all refused here rather
    /// than discovered at the sample loop.
    ///
    /// Expansion runs **before** the gate is checked, because a step string's
    /// gate defaults to its step length: a `div` that is not a length would
    /// otherwise be reported as a bad `dur`, naming a field the document does
    /// not have.
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

/// One curve's own points: that there are some, that they ascend, and that
/// each is a number the parameter can take.
/// The first `humanize` amount that is not an amount — negative, or not a
/// number — as its place among `timing`, `velocity` and `timbre`, and its
/// value. A scatter is a magnitude, so a negative one is not the other
/// direction, it is nonsense.
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
    // Last, so a NaN beat is reported as the number it is not rather than as
    // an ordering it happens to break.
    if !curve.is_sorted() {
        return Err(bad_curve("`beat` must ascend, and no two may be equal"));
    }
    Ok(())
}

/// What can only be checked once every track's patch is in hand.
///
/// A `cutoff` curve on an instrument with no filter has nothing to move, and a
/// curve that moves nothing is the failure this whole check exists to prevent.
/// It cannot be caught in [`Song::validate`] because a track may name its patch
/// rather than carry it, and this crate does not resolve names.
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
