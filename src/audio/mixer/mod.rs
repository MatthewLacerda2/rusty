//! src/audio/mixer/ — the audio mixer: groups, snapshots and ducking (#465).
//!
//! Unity's `AudioMixer`, cut down. Every voice routes through one named **group**
//! (a bus); groups form a tree under `Master`, and each has a volume, a mute, a
//! low-pass and a high-pass filter and a reverb send ([`GroupSettings`]). A
//! **snapshot** is a named partial set of group settings the game transitions to
//! over a duration ("BulletTime", "Flashbanged", "PauseMenu"); a **duck** lowers one
//! group while another plays.
//!
//! This is the sim-side half: pure data stepped by sim time (`Time.unscaledTime`, so
//! a transition runs through slow-mo and a `timeScale` 0 pause menu), so headless
//! runs and replays read back the same mix. The device renders it: every group is a
//! kira track (`device/groups.rs`); the `NullBackend` simply keeps the resolved
//! state for read-back.

mod duck;
mod maestro;
pub mod settings;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use duck::Duck;
pub use settings::{Filter, GroupMix, GroupPatch, GroupSettings, GroupState};

/// A group's index in the mixer. Stable for the mixer's life; `Master` is `0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupId(pub u16);

impl GroupId {
    /// The root group every other one sums into.
    pub const MASTER: GroupId = GroupId(0);
}

/// The groups every mixer starts with: Master, and the five buses under it.
pub const DEFAULT_GROUPS: [&str; 6] = ["Master", "Music", "SFX", "Voice", "World", "UI"];

/// One group in the tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    /// `None` only for Master.
    pub parent: Option<GroupId>,
    pub settings: GroupSettings,
}

/// A snapshot transition in flight: every group's settings at its start and end.
#[derive(Clone, Debug, PartialEq)]
struct Transition {
    from: Vec<GroupSettings>,
    to: Vec<GroupSettings>,
    start: f64,
    seconds: f64,
}

impl Transition {
    /// How far along it is at `now`, in `[0, 1]`.
    fn progress(&self, now: f64) -> f32 {
        if self.seconds <= 0.0 {
            return 1.0;
        }
        ((now - self.start) / self.seconds).clamp(0.0, 1.0) as f32
    }
}

/// The whole mixer. See the module docs.
#[derive(Clone, Debug)]
pub struct Mixer {
    groups: Vec<Group>,
    snapshots: BTreeMap<String, Vec<(GroupId, GroupPatch)>>,
    transition: Option<Transition>,
    /// The snapshot last transitioned to, for read-back.
    snapshot: Option<String>,
    ducks: Vec<Duck>,
    /// The sim time of the last [`Mixer::advance`].
    now: f64,
}

impl Default for Mixer {
    fn default() -> Self {
        let mut groups = vec![Group {
            name: DEFAULT_GROUPS[0].to_string(),
            parent: None,
            settings: GroupSettings::default(),
        }];
        for name in &DEFAULT_GROUPS[1..] {
            groups.push(Group {
                name: name.to_string(),
                parent: Some(GroupId::MASTER),
                settings: GroupSettings::default(),
            });
        }
        Self {
            groups,
            snapshots: BTreeMap::new(),
            transition: None,
            snapshot: None,
            ducks: Vec::new(),
            now: 0.0,
        }
    }
}

impl Mixer {
    /// Every group, indexed by [`GroupId`]; parents always come before children.
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// The group called `name` (exact match).
    pub fn find(&self, name: &str) -> Option<GroupId> {
        let index = self.groups.iter().position(|g| g.name == name)?;
        Some(GroupId(index as u16))
    }

    /// `name`'s group, or an error naming the groups that do exist.
    pub fn require(&self, name: &str) -> Result<GroupId, String> {
        self.find(name).ok_or_else(|| {
            let names: Vec<_> = self.groups.iter().map(|g| g.name.as_str()).collect();
            format!("no mixer group '{name}' (groups: {})", names.join(", "))
        })
    }

    /// Add a group called `name` under `parent`. Errs if the name is taken.
    pub fn create(&mut self, name: &str, parent: GroupId) -> Result<GroupId, String> {
        if name.is_empty() || self.find(name).is_some() {
            return Err(format!("mixer group name '{name}' is empty or taken"));
        }
        let id = GroupId(u16::try_from(self.groups.len()).map_err(|_| "too many groups")?);
        self.groups.push(Group {
            name: name.to_string(),
            parent: Some(parent),
            settings: GroupSettings::default(),
        });
        Ok(id)
    }

    /// Set the fields `patch` names on group `id`, now. A transition in flight keeps
    /// them where they were put rather than blending them away.
    pub fn set(&mut self, id: GroupId, patch: &GroupPatch) {
        let i = usize::from(id.0);
        self.groups[i].settings = patch.apply(&self.groups[i].settings);
        if let Some(t) = &mut self.transition {
            for side in [&mut t.from, &mut t.to] {
                if let Some(s) = side.get_mut(i) {
                    *s = patch.apply(s);
                }
            }
        }
    }

    /// Store snapshot `name` (replacing one of the same name).
    pub fn define_snapshot(&mut self, name: &str, patches: Vec<(GroupId, GroupPatch)>) {
        self.snapshots.insert(name.to_string(), patches);
    }

    /// Begin blending every group toward snapshot `name` over `seconds`, starting at
    /// sim time `now`. A zero duration lands at once.
    pub fn transition_to(&mut self, name: &str, seconds: f64, now: f64) -> Result<(), String> {
        let patches = self.snapshots.get(name).ok_or_else(|| {
            let names: Vec<_> = self.snapshots.keys().map(String::as_str).collect();
            format!("no snapshot '{name}' (snapshots: {})", names.join(", "))
        })?;
        let from: Vec<GroupSettings> = self.groups.iter().map(|g| g.settings).collect();
        let mut to = from.clone();
        for (id, patch) in patches {
            if let Some(s) = to.get_mut(usize::from(id.0)) {
                *s = patch.apply(s);
            }
        }
        self.transition = Some(Transition {
            from,
            to,
            start: now,
            seconds: seconds.max(0.0),
        });
        self.snapshot = Some(name.to_string());
        self.step_transition(now);
        Ok(())
    }

    /// The snapshot last transitioned to, and how far along that blend is.
    pub fn snapshot(&self) -> Option<(&str, f32)> {
        let progress = self
            .transition
            .as_ref()
            .map_or(1.0, |t| t.progress(self.now));
        Some((self.snapshot.as_deref()?, progress))
    }

    /// Add a duck rule.
    pub fn add_duck(&mut self, duck: Duck) {
        self.ducks.push(duck);
    }

    /// Drop every duck rule (their targets recover at once).
    pub fn clear_ducks(&mut self) {
        self.ducks.clear();
    }

    /// Step the mixer to sim time `now`: the snapshot blend, then every duck, with
    /// `playing(group)` saying whether a live voice routes into that group itself.
    pub fn advance(&mut self, now: f64, playing: impl Fn(GroupId) -> bool) {
        let dt = (now - self.now).max(0.0) as f32;
        self.now = now;
        self.step_transition(now);
        let active: Vec<bool> = (0..self.groups.len())
            .map(|i| self.subtree_playing(GroupId(i as u16), &playing))
            .collect();
        for duck in &mut self.ducks {
            duck.step(active[usize::from(duck.trigger.0)], dt);
        }
    }

    fn step_transition(&mut self, now: f64) {
        let Some(t) = &self.transition else { return };
        let progress = t.progress(now);
        for (i, (from, to)) in t.from.iter().zip(&t.to).enumerate() {
            self.groups[i].settings = from.lerp(to, progress);
        }
        if progress >= 1.0 {
            self.transition = None;
        }
    }

    /// Whether `id` or any group under it has a playing voice.
    fn subtree_playing(&self, id: GroupId, playing: &impl Fn(GroupId) -> bool) -> bool {
        (0..self.groups.len())
            .map(|i| GroupId(i as u16))
            .any(|g| playing(g) && self.is_within(g, id))
    }

    /// Whether `g` is `ancestor` or sits under it.
    fn is_within(&self, mut g: GroupId, ancestor: GroupId) -> bool {
        loop {
            if g == ancestor {
                return true;
            }
            match self.groups[usize::from(g.0)].parent {
                Some(p) => g = p,
                None => return false,
            }
        }
    }

    /// The gain group `id`'s ducks apply right now.
    pub fn duck_gain(&self, id: GroupId) -> f32 {
        self.ducks
            .iter()
            .filter(|d| d.target == id)
            .map(Duck::gain)
            .product()
    }

    /// What the device applies to group `id`'s track.
    pub fn mix(&self, id: GroupId) -> GroupMix {
        GroupMix::new(&self.groups[usize::from(id.0)].settings, self.duck_gain(id))
    }

    /// Group `id` as the agent reads it back.
    pub fn state(&self, id: GroupId) -> GroupState {
        let group = &self.groups[usize::from(id.0)];
        let mut effective = 1.0;
        let mut at = Some(id);
        while let Some(g) = at {
            effective *= self.mix(g).volume;
            at = self.groups[usize::from(g.0)].parent;
        }
        GroupState {
            name: group.name.clone(),
            parent: group
                .parent
                .map(|p| self.groups[usize::from(p.0)].name.clone()),
            settings: group.settings,
            duck: self.duck_gain(id),
            effective_volume: effective,
        }
    }
}

#[cfg(test)]
#[path = "mixer_tests.rs"]
mod mixer_tests;
