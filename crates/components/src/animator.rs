use serde::{Deserialize, Serialize};

mod graph_state;
mod ik;
mod montage;
mod parameters;
mod playback;
mod playhead;
mod root_motion;
mod states;
mod sync;
pub use ik::{IkChain, IkConstraint, IkTarget};
pub use montage::{
    ActiveMontage, MontageEvent, MontageRefused, Montages, MONTAGE_BLEND_OUT, MONTAGE_END,
};
pub use parameters::{AnimatorParameter, AnimatorParameters};
pub use playback::{Motion, Playback};
pub use playhead::{wrap_delta, Bounds, Playhead, Step};
pub use root_motion::{root_chain, RootMotion, RootMotionState};
pub use states::{NodeStates, Reported, StateCallback, StateEvent, StateInfo, StateTracker};
pub use sync::{follow_clamped, stored_leaf, LeafTime, SyncSource, SyncState};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorComponent {
    #[serde(flatten)]
    pub base: Playback,
    pub speed: f32,
    pub is_playing: bool,
    pub freeze: bool,
    #[serde(default, skip_serializing_if = "AnimatorParameters::is_empty")]
    pub parameters: AnimatorParameters,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub graph_enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ik: Vec<IkConstraint>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub apply_root_motion: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub root_bone: String,
    /// The last step's root motion and what it moved (`Animator.GetRootMotion`);
    /// runtime only, never saved.
    #[serde(skip)]
    pub root_motion: RootMotionState,
    /// The montages playing in its slots; runtime only, never saved,
    /// so a load or a Play/Stop restore leaves every slot empty.
    #[serde(skip)]
    pub montages: Montages,
    /// The graph states last reported to the scripts; runtime only.
    #[serde(skip)]
    pub states: StateTracker,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerState {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub weight: f32,
    #[serde(flatten)]
    pub playback: Playback,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

impl Default for AnimatorComponent {
    fn default() -> Self {
        Self {
            base: Playback::default(),
            speed: 1.0,
            is_playing: false,
            freeze: false,
            parameters: AnimatorParameters::new(),
            graph: None,
            graph_enabled: true,
            layers: Vec::new(),
            ik: Vec::new(),
            apply_root_motion: false,
            root_bone: String::new(),
            root_motion: RootMotionState::default(),
            montages: Montages::default(),
            states: StateTracker::default(),
        }
    }
}

impl AnimatorComponent {
    pub fn play(&mut self, clip: String) {
        self.base.play(Motion::Clip(&clip));
        self.run();
    }

    /// Crossfade the base layer into `clip` over `duration` seconds. A
    /// non-positive `duration`, or a fade into the already-current clip, degrades
    /// to a plain [`play`](Self::play).
    pub fn crossfade(&mut self, clip: String, duration: f32) {
        self.base.crossfade(Motion::Clip(&clip), duration);
        self.run();
    }

    // Test hooks for other crates' tests.
    #[doc(hidden)]
    pub fn crossfade_weight(&self) -> f32 {
        self.base.crossfade_weight()
    }

    #[doc(hidden)]
    pub fn is_crossfading(&self) -> bool {
        self.base.is_crossfading()
    }

    /// True when the playheads move this step: playing and not paused.
    pub fn is_running(&self) -> bool {
        self.is_playing && !self.freeze
    }

    pub fn ik_mut(&mut self, name: &str) -> Option<&mut IkConstraint> {
        self.ik.iter_mut().find(|c| c.name == name)
    }

    /// Keeps a same-named one's `applied`, so the next solve still undoes its writes.
    pub fn set_ik(&mut self, mut constraint: IkConstraint) {
        match self.ik_mut(&constraint.name) {
            Some(slot) => {
                constraint.applied = std::mem::take(&mut slot.applied);
                *slot = constraint;
            }
            None => self.ik.push(constraint),
        }
    }

    fn run(&mut self) {
        self.is_playing = true;
        self.freeze = false;
    }
}

impl crate::FirstClass for AnimatorComponent {
    const NAME: &'static str = "Animator";
}

#[cfg(test)]
mod graph_state_tests;
#[cfg(test)]
mod parameters_tests;
#[cfg(test)]
mod sync_tests;
#[cfg(test)]
mod tests;
