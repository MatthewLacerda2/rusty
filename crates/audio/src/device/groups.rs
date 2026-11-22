use std::sync::Arc;

use rusty_core::collections::Map;

use kira::backend::Backend;
use kira::effect::filter::{FilterBuilder, FilterHandle, FilterMode};
use kira::effect::volume_control::{VolumeControlBuilder, VolumeControlHandle};
use kira::effect::EffectBuilder;
use kira::track::{MainTrackBuilder, TrackBuilder, TrackHandle};
use kira::{AudioManager, Decibels, Mix, Tween};

use super::gate::{Gated, Wake};
use super::reverb::ReverbBus;
use rusty_components::ReverbParams;

use crate::mixer::effects::rack::{rack, RackHandle};
use crate::mixer::effects::KeyLevel;
use crate::mixer::settings::{HIGH_PASS_OFF, LOW_PASS_OFF};
use crate::mixer::{Filter, GroupId, GroupMix};

pub const VOICES_PER_GROUP: usize = 256;

/// A linear gain as kira decibels; `0` (and anything at or under −60 dB) is silence.
pub fn decibels(gain: f32) -> Decibels {
    if gain <= 0.001 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * gain.log10())
    }
}

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

struct Strip {
    low_pass: FilterHandle,
    high_pass: FilterHandle,
    volume: Option<VolumeControlHandle>,
    rack: RackHandle,
    wake: Wake,
}

impl Strip {
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

pub struct MasterStrip(Strip);

impl MasterStrip {
    pub fn build(main: &mut MainTrackBuilder) -> Self {
        Self(Strip::build(main, true))
    }
}

struct GroupTrack {
    track: TrackHandle,
    strip: Option<Strip>,
}

pub struct GroupTracks {
    tracks: Map<GroupId, GroupTrack>,
    levels: Map<GroupId, Arc<KeyLevel>>,
    reverb: ReverbBus,
    master: MasterStrip,
}

impl GroupTracks {
    pub fn new<B: Backend>(manager: &mut AudioManager<B>, master: MasterStrip) -> Option<Self> {
        Some(Self {
            tracks: Map::default(),
            levels: Map::default(),
            reverb: ReverbBus::new(manager, &ReverbParams::DRY)?,
            master,
        })
    }

    pub fn set_reverb(&mut self, params: &ReverbParams) {
        self.reverb.set(params);
    }

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

fn retune(handle: &mut FilterHandle, f: Filter, open: bool) {
    let tween = Tween::default();
    handle.set_cutoff(f64::from(f.cutoff), tween);
    handle.set_resonance(f64::from(f.resonance), tween);
    handle.set_mix(if open { Mix::DRY } else { Mix::WET }, tween);
}

#[cfg(test)]
#[path = "groups_tests.rs"]
mod groups_tests;
