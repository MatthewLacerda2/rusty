//! src/audio/mixer/maestro.rs — the maestro's mixer verbs (#465).
//!
//! What the `Audio` group / snapshot / duck API calls: each change lands in the
//! sim-side [`Mixer`] first, then the resolved [`GroupMix`] of every group it moved
//! goes to the backend, so the device and the read-back never disagree. The
//! per-frame [`AudioMaestro::advance_mixer`] runs as a `LateUpdate` system on sim
//! time (`app/audio.rs`).

use super::{Duck, GroupId, GroupPatch, GroupState, Mixer};
use crate::audio::AudioMaestro;

impl AudioMaestro {
    /// The mixer, for read-back.
    pub fn mixer(&self) -> &Mixer {
        &self.mixer
    }

    /// The group a voice naming `group` routes into: Master for an empty name, and
    /// for a name the mixer does not know (logged, since that voice skips its bus).
    pub(crate) fn route(&self, group: &str) -> GroupId {
        if group.is_empty() {
            return GroupId::MASTER;
        }
        self.mixer.find(group).unwrap_or_else(|| {
            log::warn!("[Audio] no mixer group '{group}'; the voice plays through Master");
            GroupId::MASTER
        })
    }

    /// Build every group on the backend (a fresh backend, or a reset mixer).
    pub(crate) fn sync_groups(&mut self) {
        for (i, group) in self.mixer.groups().iter().enumerate() {
            self.backend.add_group(GroupId(i as u16), group.parent);
        }
        self.push_groups();
    }

    /// Send every group's resolved mix to the backend.
    fn push_groups(&mut self) {
        for i in 0..self.mixer.groups().len() {
            let id = GroupId(i as u16);
            self.backend.set_group(id, &self.mixer.mix(id));
        }
    }

    /// Add group `name` under `parent` (Master when `None`).
    pub fn create_group(&mut self, name: &str, parent: Option<&str>) -> Result<(), String> {
        let parent = parent.map_or(Ok(GroupId::MASTER), |p| self.mixer.require(p))?;
        let id = self.mixer.create(name, parent)?;
        self.backend.add_group(id, Some(parent));
        self.backend.set_group(id, &self.mixer.mix(id));
        Ok(())
    }

    /// Set the fields `patch` names on group `name`, now.
    pub fn set_group(&mut self, name: &str, patch: &GroupPatch) -> Result<(), String> {
        let id = self.mixer.require(name)?;
        self.mixer.set(id, patch);
        self.backend.set_group(id, &self.mixer.mix(id));
        Ok(())
    }

    /// Store snapshot `name`: per group name, the fields it sets.
    pub fn define_snapshot(
        &mut self,
        name: &str,
        groups: &[(String, GroupPatch)],
    ) -> Result<(), String> {
        let patches = groups
            .iter()
            .map(|(group, patch)| Ok((self.mixer.require(group)?, *patch)))
            .collect::<Result<_, String>>()?;
        self.mixer.define_snapshot(name, patches);
        Ok(())
    }

    /// Blend toward snapshot `name` over `seconds`, from sim time `now`.
    pub fn transition_to_snapshot(
        &mut self,
        name: &str,
        seconds: f64,
        now: f64,
    ) -> Result<(), String> {
        self.mixer.transition_to(name, seconds, now)?;
        self.push_groups();
        Ok(())
    }

    /// Duck group `target` to `volume` while group `trigger` plays.
    pub fn add_duck(
        &mut self,
        trigger: &str,
        target: &str,
        volume: f32,
        attack: f32,
        release: f32,
    ) -> Result<(), String> {
        let (trigger, target) = (self.mixer.require(trigger)?, self.mixer.require(target)?);
        self.mixer
            .add_duck(Duck::new(trigger, target, volume, attack, release));
        Ok(())
    }

    /// Drop every duck rule.
    pub fn clear_ducks(&mut self) {
        self.mixer.clear_ducks();
        self.push_groups();
    }

    /// Group `name` as the agent reads it back.
    pub fn group_state(&self, name: &str) -> Option<GroupState> {
        self.mixer.find(name).map(|id| self.mixer.state(id))
    }

    /// Step the mixer to sim time `now` (snapshot blends, ducks) and send the groups
    /// whose mix moved. A group "plays" while a live voice routes into it.
    pub fn advance_mixer(&mut self, now: f64) {
        let before: Vec<_> = (0..self.mixer.groups().len())
            .map(|i| self.mixer.mix(GroupId(i as u16)))
            .collect();
        let mut playing = vec![false; before.len()];
        let groups = self.entity_voices.values().map(|v| v.group);
        for g in groups.chain(self.oneshots.values().map(|s| s.group)) {
            playing[usize::from(g.0)] = true;
        }
        self.mixer.advance(now, |g| playing[usize::from(g.0)]);
        for (i, old) in before.iter().enumerate() {
            let id = GroupId(i as u16);
            let mix = self.mixer.mix(id);
            if mix != *old {
                self.backend.set_group(id, &mix);
            }
        }
    }

    /// Back to the default mixer.
    pub fn reset_mixer(&mut self) {
        self.mixer = Mixer::default();
        self.sync_groups();
    }

    /// Leaving Play: silence every voice and discard play-mode mixer changes, as
    /// Stop discards the scene's.
    pub fn exit_play(&mut self) {
        self.stop_all();
        self.reset_mixer();
    }
}

#[cfg(test)]
#[path = "maestro_tests.rs"]
mod maestro_tests;
