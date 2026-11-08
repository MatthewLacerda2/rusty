//! src/components/animator/montage.rs — the montages an animator plays (#1894).
//!
//! Runtime only, like an IK target: what a script just did is never saved, so a
//! scene load, a Play/Stop restore and a change of graph all leave every slot
//! empty, firing nothing. Each [`ActiveMontage`] carries a copy of what it
//! plays (its montage, its slot's mask and place, its clip's length), taken
//! when it starts, so the sampler needs no graph and a graph edited mid-play
//! changes nothing already playing.
//!
//! One montage per slot is *active*; an interrupted or stopped one blends out
//! beside it from the weight it had (Unreal's `Montage_Play` and
//! `Montage_Stop`). The two engine events, [`MONTAGE_BLEND_OUT`] and
//! [`MONTAGE_END`], fire once each per montage, also when it is interrupted,
//! each carrying a [`MontageEvent`] that says whose it is; what a script call
//! starts is queued in `pending` and fires on the next step. A stopped or
//! interrupted montage's own markers fall silent as it fades (a faded-out
//! motion is silent); one blending out at its own end still fires them.
//! Advancing the playheads is `app::animation::montage`'s.

use crate::asset::animation_graph::{GraphSlot, Montage, MontageSection};

mod resolve;
pub use resolve::MontageRefused;

/// Fired when a montage starts blending out: it finished, or was stopped or
/// interrupted (Unreal's `OnMontageBlendingOut`).
pub const MONTAGE_BLEND_OUT: &str = "MontageBlendOut";
/// Fired when a montage has fully blended out and is gone (`OnMontageEnded`).
pub const MONTAGE_END: &str = "MontageEnd";
/// The one section of a montage that declares none.
const DEFAULT_SECTION: &str = "Default";

/// Which montage an engine event belongs to (Unreal's
/// `OnMontageEnded(Montage, bInterrupted)`): the third argument
/// `OnAnimationEvent` gets for [`MONTAGE_BLEND_OUT`] and [`MONTAGE_END`].
#[derive(Clone, Debug, PartialEq)]
pub struct MontageEvent {
    pub montage: String,
    pub slot: String,
    /// It was stopped or interrupted, rather than played to its end.
    pub interrupted: bool,
}

/// Every montage an animator plays, and the engine events a script call queued.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Montages {
    /// In slot-stack order (`ActiveMontage::order`), and within a slot in
    /// start order: its blending-out montages before its active one. The pose
    /// and the root motion both stack in this order.
    pub playing: Vec<ActiveMontage>,
    /// [`MONTAGE_BLEND_OUT`]s a call queued, fired on the next step.
    pub pending: Vec<MontageEvent>,
}

/// A montage starting to blend out: from `from` weight to 0 over `duration`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendOut {
    pub from: f32,
    pub elapsed: f32,
    pub duration: f32,
    /// Stopped or interrupted, not ending on its own: its markers are silent.
    pub interrupted: bool,
}

/// One montage playing in a slot.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveMontage {
    /// What plays; its sections never empty (a montage declaring none gets
    /// one `"Default"` section spanning the clip).
    pub montage: Montage,
    pub slot: GraphSlot,
    /// The slot's place in the slot stack (`AnimationGraph::slot`).
    pub order: usize,
    /// The clip's length in seconds.
    pub length: f32,
    /// Play rate, stacked on the animator's speed.
    pub rate: f32,
    /// Clip seconds.
    pub time: f32,
    /// The index of the section playing.
    pub section: usize,
    /// Each section's next, by index (`Animator.SetNextSection` rewires it).
    pub next: Vec<Option<usize>>,
    /// Unscaled seconds since it started: what blending in reads.
    pub elapsed: f32,
    pub out: Option<BlendOut>,
}

impl ActiveMontage {
    /// Section `index`'s start, within the clip.
    pub fn start(&self, index: usize) -> f32 {
        let at = self.montage.sections.get(index).map_or(0.0, |s| s.time);
        at.clamp(0.0, self.length.max(0.0))
    }

    /// Section `index`'s end: the next section's start, the last one's the
    /// clip's end.
    pub fn end(&self, index: usize) -> f32 {
        let end = match index + 1 < self.montage.sections.len() {
            true => self.start(index + 1),
            false => self.length.max(0.0),
        };
        end.max(self.start(index))
    }

    /// The section named `name`'s index.
    pub fn section_index(&self, name: &str) -> Option<usize> {
        find(&self.montage.sections, name)
    }

    /// The name of the section playing.
    pub fn section_name(&self) -> &str {
        self.montage
            .sections
            .get(self.section)
            .map_or("", |s| s.name.as_str())
    }

    /// The weight it shows at: ramping up over `blend_in`, then down from
    /// where it was over its blend out.
    pub fn weight(&self) -> f32 {
        if let Some(out) = self.out {
            if out.duration <= 0.0 {
                return 0.0;
            }
            return (out.from * (1.0 - out.elapsed / out.duration)).clamp(0.0, 1.0);
        }
        match self.montage.blend_in > 0.0 {
            true => (self.elapsed / self.montage.blend_in).clamp(0.0, 1.0),
            false => 1.0,
        }
    }

    /// Start blending out over `duration` from the weight it shows now,
    /// `interrupted` when a call stopped it. `false` when it already is.
    pub fn blend_out(&mut self, duration: f32, interrupted: bool) -> bool {
        if self.out.is_some() {
            return false;
        }
        self.out = Some(BlendOut {
            from: self.weight(),
            elapsed: 0.0,
            duration: duration.max(0.0),
            interrupted,
        });
        true
    }

    /// Whether its authored markers fire: not once it was stopped or
    /// interrupted.
    pub fn fires_markers(&self) -> bool {
        !self.out.is_some_and(|o| o.interrupted)
    }

    /// Whose an engine event of this montage is.
    pub fn event(&self) -> MontageEvent {
        MontageEvent {
            montage: self.montage.name.clone(),
            slot: self.slot.name.clone(),
            interrupted: self.out.is_some_and(|o| o.interrupted),
        }
    }
}

fn find(sections: &[MontageSection], name: &str) -> Option<usize> {
    sections.iter().position(|s| s.name == name)
}

impl Montages {
    /// The active (not blending-out) montage in `slot`, or in any slot.
    pub fn active(&self, slot: Option<&str>) -> Option<&ActiveMontage> {
        self.playing
            .iter()
            .find(|m| m.out.is_none() && slot.is_none_or(|s| m.slot.name == s))
    }

    fn active_mut(&mut self, slot: Option<&str>) -> Option<&mut ActiveMontage> {
        self.playing
            .iter_mut()
            .find(|m| m.out.is_none() && slot.is_none_or(|s| m.slot.name == s))
    }

    /// Start `montage`: whatever its slot plays blends out over the newcomer's
    /// blend-in time (Unreal's interrupt), from the weight it had. It goes
    /// last among its slot's, keeping `playing` in slot order.
    pub fn play(&mut self, montage: ActiveMontage) {
        let blend = montage.montage.blend_in;
        let slot = montage.slot.name.clone();
        self.stop(Some(&slot), Some(blend));
        let at = self.playing.partition_point(|m| m.order <= montage.order);
        self.playing.insert(at, montage);
    }

    /// Blend out the active montage in `slot` (every slot's when `None`) over
    /// `blend` seconds (its own blend out when `None`). `true` when one was.
    pub fn stop(&mut self, slot: Option<&str>, blend: Option<f32>) -> bool {
        let mut stopped = false;
        for m in &mut self.playing {
            if slot.is_some_and(|s| m.slot.name != s) {
                continue;
            }
            let duration = blend.unwrap_or(m.montage.blend_out);
            if m.blend_out(duration, true) {
                self.pending.push(m.event());
                stopped = true;
            }
        }
        stopped
    }

    /// Jump the active montage in `slot` (else the first active one with such
    /// a section) to the start of `section`. `false` when none has it.
    pub fn jump(&mut self, section: &str, slot: Option<&str>) -> bool {
        let Some(m) = self.with_section(section, slot) else {
            return false;
        };
        let Some(index) = m.section_index(section) else {
            return false;
        };
        m.section = index;
        m.time = m.start(index);
        true
    }

    /// Make section `from` go on to `to` (none, ending the montage, when
    /// empty) in the active montage of `slot` (else the first active one with
    /// a section `from`). `false` when either section is unknown.
    pub fn set_next(&mut self, from: &str, to: &str, slot: Option<&str>) -> bool {
        let Some(m) = self.with_section(from, slot) else {
            return false;
        };
        let target = match to.is_empty() {
            true => None,
            false => match m.section_index(to) {
                Some(i) => Some(i),
                None => return false,
            },
        };
        let Some(index) = m.section_index(from) else {
            return false;
        };
        m.next[index] = target;
        true
    }

    fn with_section(&mut self, section: &str, slot: Option<&str>) -> Option<&mut ActiveMontage> {
        if slot.is_some() {
            return self.active_mut(slot);
        }
        self.playing
            .iter_mut()
            .find(|m| m.out.is_none() && m.section_index(section).is_some())
    }

    /// Empty every slot, firing nothing: a load, a restore, a new graph.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_empty(&self) -> bool {
        self.playing.is_empty() && self.pending.is_empty()
    }
}
