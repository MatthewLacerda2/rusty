//! src/audio/device/groups.rs — mixer groups as kira tracks (#465).
//!
//! Every mixer group is one kira sub-track, nested as the group tree is (a child
//! track sums into its parent's, Master's into kira's main track), and routes a
//! send into the one reverb bus (`reverb.rs`, tuned by reverb zones, #469). What a
//! group runs on its signal is its `Strip`: a low-pass then a high-pass filter
//! (kira's state-variable filter; an "open" filter is switched fully dry, so it is
//! bit-transparent), then its effect chain in its rack (`mixer/effects/rack.rs`,
//! #1748), which also publishes the group's level for sidechains. The track's
//! volume (mute and duck included) comes last.
//!
//! **Master's strip is on kira's main track** (#1855): kira sums a send track only
//! into its main track, never into a sub-track, so the reverb bus adds up beside
//! Master's track, not through it. Master's whole strip therefore runs on the main
//! track ([`MasterStrip`]), after Master's track and the reverb bus have summed:
//! its filters, then its volume and mute, then its rack, then the speaker-mode
//! stage (`output.rs`). So Master's volume, mute, filters and default limiter hold
//! the whole mix, reverb included, and Master's own track passes through (it is
//! kept for the voices played into Master and their send). Volume, cutoffs and the
//! send move with kira's 10 ms tween, so a snapshot stepped once a frame never
//! clicks.

use std::sync::Arc;

use crate::core::collections::Map;

use kira::backend::Backend;
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::effect::volume_control::{VolumeControlBuilder, VolumeControlHandle};
use kira::effect::EffectBuilder;
use kira::track::{MainTrackBuilder, TrackBuilder, TrackHandle};
use kira::{AudioManager, Decibels, Mix, Tween};

use super::gate::{Gated, Wake};
use super::reverb::ReverbBus;
use crate::components::ReverbParams;

use crate::audio::mixer::effects::rack::{rack, RackHandle};
use crate::audio::mixer::effects::KeyLevel;
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

/// A track builder a [`Strip`] adds its effects to: a group's or kira's main one.
trait Effects {
    fn add<E: EffectBuilder>(&mut self, effect: E) -> E::Handle;
}

impl Effects for TrackBuilder {
    fn add<E: EffectBuilder>(&mut self, effect: E) -> E::Handle {
        self.add_effect(effect)
    }
}

impl Effects for MainTrackBuilder {
    fn add<E: EffectBuilder>(&mut self, effect: E) -> E::Handle {
        self.add_effect(effect)
    }
}

/// What a group runs on its signal, in order (module docs), one [`Wake`] for all.
struct Strip {
    low_pass: FilterHandle,
    high_pass: FilterHandle,
    /// Master's volume, before its rack; `None` for a group whose track's own
    /// volume runs after its rack.
    volume: Option<VolumeControlHandle>,
    rack: RackHandle,
    wake: Wake,
}

impl Strip {
    /// Add the strip's effects to `track`; `volume` adds a volume before the rack.
    fn build(track: &mut impl Effects, volume: bool) -> Self {
        let wake = Wake::default();
        let gated = |mode, f| Gated(filter(mode, f), wake.clone(), None);
        let low_pass = track.add(gated(FilterMode::LowPass, Filter::LOW_PASS_OPEN));
        let high_pass = track.add(gated(FilterMode::HighPass, Filter::HIGH_PASS_OPEN));
        // Ungated: a gain costs less than the gate's silence scan would.
        let volume = volume.then(|| track.add(VolumeControlBuilder::new(Decibels::IDENTITY)));
        let (effects, rack) = rack();
        track.add(Gated(effects, wake.clone(), Some(rack.tail())));
        Self {
            low_pass,
            high_pass,
            volume,
            rack,
            wake,
        }
    }

    /// Retune to `mix`; `key` finds a group's level for a sidechain. Without a
    /// volume of its own, the rack's published level is scaled by the track's.
    fn set(&mut self, mix: &GroupMix, key: &dyn Fn(GroupId) -> Option<Arc<KeyLevel>>) {
        let low_open = mix.low_pass.cutoff >= LOW_PASS_OFF;
        retune(&mut self.low_pass, mix.low_pass, low_open);
        let high_open = mix.high_pass.cutoff <= HIGH_PASS_OFF;
        retune(&mut self.high_pass, mix.high_pass, high_open);
        match self.volume.as_mut() {
            Some(volume) => volume.set_volume(decibels(mix.volume), Tween::default()),
            None => self.rack.set_output_gain(mix.volume),
        }
        self.rack.set(&mix.effects, key);
        self.wake.ring();
    }
}

/// Master's strip (`Strip`), on kira's main track (module docs).
pub struct MasterStrip(Strip);

impl MasterStrip {
    /// Add the strip to `main`; call it before the speaker-mode stage is added.
    pub fn build(main: &mut MainTrackBuilder) -> Self {
        Self(Strip::build(main, true))
    }
}

/// One group's track, and its strip (`None` for Master: the [`MasterStrip`]).
struct GroupTrack {
    track: TrackHandle,
    strip: Option<Strip>,
}

/// Every group's track plus the reverb bus they send to.
pub struct GroupTracks {
    tracks: Map<GroupId, GroupTrack>,
    /// Every group's output level, for the sidechains that key from it.
    levels: Map<GroupId, Arc<KeyLevel>>,
    reverb: ReverbBus,
    master: MasterStrip,
}

impl GroupTracks {
    /// The reverb bus (dry: no zone yet), ready for groups to be added; `master`
    /// runs Master's strip.
    pub fn new<B: Backend>(manager: &mut AudioManager<B>, master: MasterStrip) -> Option<Self> {
        Some(Self {
            tracks: Map::default(),
            levels: Map::default(),
            reverb: ReverbBus::new(manager, &ReverbParams::DRY)?,
            master,
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
        let strip = (id != GroupId::MASTER).then(|| Strip::build(&mut builder, false));
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
        let rack = strip.as_ref().map_or(&self.master.0.rack, |s| &s.rack);
        self.levels.insert(id, rack.level());
        self.tracks.insert(id, GroupTrack { track, strip });
    }

    /// Apply `mix` to group `id`: its track's send, and its strip (Master's on
    /// the main track, where its volume runs too).
    pub fn set(&mut self, id: GroupId, mix: &GroupMix) {
        let reverb = self.reverb.id();
        let Some(group) = self.tracks.get_mut(&id) else {
            return;
        };
        let tween = Tween::default();
        group
            .track
            .set_send(reverb, decibels(mix.reverb_send), tween)
            .ok();
        let levels = &self.levels;
        let key = |g| levels.get(&g).cloned();
        match group.strip.as_mut() {
            Some(strip) => {
                group.track.set_volume(decibels(mix.volume), tween);
                strip.set(mix, &key);
            }
            None => self.master.0.set(mix, &key),
        }
    }

    /// Once a frame: let every rack drop what its audio thread handed back and
    /// rebuild after a sample-rate change.
    pub fn maintain(&mut self) {
        let levels = &self.levels;
        let key = |g| levels.get(&g).cloned();
        for strip in self.tracks.values_mut().filter_map(|g| g.strip.as_mut()) {
            strip.rack.maintain(&key);
        }
        self.master.0.rack.maintain(&key);
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
