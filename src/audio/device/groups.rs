//! src/audio/device/groups.rs — mixer groups as kira tracks (#465).
//!
//! Every mixer group is one kira sub-track, nested as the group tree is (a child
//! track sums into its parent's, Master's into kira's main track). Each track runs
//! a low-pass then a high-pass filter (kira's state-variable filter; an "open"
//! filter is switched fully dry, so it is bit-transparent) and routes a send into
//! the one reverb bus (`reverb.rs`, tuned by reverb zones, #469). Volume,
//! cutoffs and the send move with kira's 10 ms tween, so a snapshot stepped once a
//! frame never clicks.

use std::collections::HashMap;

use kira::backend::Backend;
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, Decibels, Mix, Tween};

use super::reverb::ReverbBus;
use crate::components::ReverbParams;

use crate::audio::mixer::settings::{HIGH_PASS_OFF, LOW_PASS_OFF};
use crate::audio::mixer::{Filter, GroupId, GroupMix};

/// How many voices one group's track can play at once.
pub const VOICES_PER_GROUP: usize = 256;

/// A linear gain as kira decibels; `0` (and anything at or under −60 dB) is silence.
pub fn decibels(gain: f32) -> Decibels {
    if gain <= 0.001 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * gain.log10())
    }
}

/// One group's track and the handles that retune it.
pub struct GroupTrack {
    pub track: TrackHandle,
    low_pass: FilterHandle,
    high_pass: FilterHandle,
}

/// Every group's track plus the reverb bus they send to.
pub struct GroupTracks {
    tracks: HashMap<GroupId, GroupTrack>,
    reverb: ReverbBus,
}

impl GroupTracks {
    /// The reverb bus (dry: no zone yet), ready for groups to be added.
    pub fn new<B: Backend>(manager: &mut AudioManager<B>) -> Option<Self> {
        Some(Self {
            tracks: HashMap::new(),
            reverb: ReverbBus::new(manager, &ReverbParams::DRY)?,
        })
    }

    /// Retune the reverb bus to the listener's zone blend.
    pub fn set_reverb(&mut self, params: &ReverbParams) {
        self.reverb.set(params);
    }

    /// Build group `id`'s track under `parent` (kira's main track for `None`),
    /// replacing any track already at `id`. Logs and skips when kira is out of
    /// tracks or the parent is missing.
    pub fn add<B: Backend>(
        &mut self,
        manager: &mut AudioManager<B>,
        id: GroupId,
        parent: Option<GroupId>,
    ) {
        self.tracks.remove(&id);
        let mut builder = TrackBuilder::new()
            .sound_capacity(VOICES_PER_GROUP)
            .with_send(self.reverb.id(), Decibels::SILENCE);
        let low_pass = builder.add_effect(filter(FilterMode::LowPass, Filter::LOW_PASS_OPEN));
        let high_pass = builder.add_effect(filter(FilterMode::HighPass, Filter::HIGH_PASS_OPEN));
        let track = match parent {
            None => manager.add_sub_track(builder).ok(),
            Some(p) => self
                .tracks
                .get_mut(&p)
                .and_then(|p| p.track.add_sub_track(builder).ok()),
        };
        let Some(track) = track else {
            log::warn!("[Audio] mixer group {id:?} got no track; its voices play silent");
            return;
        };
        let group = GroupTrack {
            track,
            low_pass,
            high_pass,
        };
        self.tracks.insert(id, group);
    }

    /// Apply `mix` to group `id`'s track.
    pub fn set(&mut self, id: GroupId, mix: &GroupMix) {
        let reverb = self.reverb.id();
        let Some(group) = self.tracks.get_mut(&id) else {
            return;
        };
        let tween = Tween::default();
        group.track.set_volume(decibels(mix.volume), tween);
        group
            .track
            .set_send(reverb, decibels(mix.reverb_send), tween)
            .ok();
        retune(
            &mut group.low_pass,
            mix.low_pass,
            mix.low_pass.cutoff >= LOW_PASS_OFF,
        );
        retune(
            &mut group.high_pass,
            mix.high_pass,
            mix.high_pass.cutoff <= HIGH_PASS_OFF,
        );
    }

    /// Group `id`'s track, if it was built.
    pub fn track(&mut self, id: GroupId) -> Option<&mut TrackHandle> {
        self.tracks.get_mut(&id).map(|g| &mut g.track)
    }
}

/// A filter of `mode` at `f`, dry (open) until a mix says otherwise.
fn filter(mode: FilterMode, f: Filter) -> FilterBuilder {
    FilterBuilder::new()
        .mode(mode)
        .cutoff(f64::from(f.cutoff))
        .resonance(f64::from(f.resonance))
        .mix(Mix::DRY)
}

/// Move a filter to `f`; an `open` filter is switched fully dry.
fn retune(handle: &mut FilterHandle, f: Filter, open: bool) {
    let tween = Tween::default();
    handle.set_cutoff(f64::from(f.cutoff), tween);
    handle.set_resonance(f64::from(f.resonance), tween);
    handle.set_mix(if open { Mix::DRY } else { Mix::WET }, tween);
}

#[cfg(test)]
#[path = "groups_tests.rs"]
mod groups_tests;
