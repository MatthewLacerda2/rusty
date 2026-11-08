//! src/components/animator/states.rs — which graph states are active, and what
//! the scripts were last told about them (#1901).
//!
//! A layer has up to two active states: the node it is in and, while a
//! crossfade blends, the node it is leaving. [`NodeStates`] keeps the second
//! one's name next to the playback, and counts entries so that re-entering
//! the node already playing (a self-transition, `PlayNode` on the current
//! node) reads as leaving one state and entering another, as in Unity.
//!
//! The animate system compares those against the [`StateTracker`] — the states
//! last reported to the scripts, per layer — and queues a [`StateCallback`] for
//! each difference: `OnStateExit` for a state no longer active, `OnStateEnter`
//! for a new one, `OnStateUpdate` for one still active. Both trackers are
//! runtime state: a load or a Play enter starts with none reported, so every
//! active state is entered afresh.

/// The graph-state bookkeeping a [`Playback`](super::Playback) carries beside
/// its motion (#1901); runtime only, never saved (a load mid-crossfade keeps
/// only the node it is entering as a state).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeStates {
    /// The graph node a crossfade is leaving, while it blends. `None` outside
    /// one, and when the motion fading out is not a node's (`Animator.Crossfade`).
    pub previous_node: Option<String>,
    /// The active node's tag (Unity's state *Tag*), refreshed from the graph
    /// by each animation step.
    pub tag: Option<String>,
    /// Which entry of the current node this is: bumped by every node entry.
    pub entry: u32,
    /// The entry of `previous_node`.
    pub previous_entry: u32,
    /// The current playhead's travel in its last step (playhead units), until
    /// the state diff takes it.
    pub travel: f32,
}

impl NodeStates {
    /// A node tagged `tag` (empty: none) was entered from `left` (the node
    /// active before it, if any); `fading` when a crossfade now blends out of it.
    pub fn entered(&mut self, left: Option<String>, tag: &str, fading: bool) {
        self.previous_node = left.filter(|_| fading);
        self.previous_entry = self.entry;
        self.entry = self.entry.wrapping_add(1);
        self.tag = (!tag.is_empty()).then(|| tag.to_string());
    }
}

/// One graph state as the scripts see it: Unity's `AnimatorStateInfo`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateInfo {
    /// The layer's index: 0 is the base layer, 1.. the extra layers.
    pub layer: usize,
    /// The layer's name: `"Base Layer"` for layer 0, as in Unity.
    pub layer_name: String,
    /// The node's name.
    pub state: String,
    /// The node's tag, if it has one.
    pub tag: Option<String>,
    /// Seconds of the state's motion played since it was entered (the clip's
    /// own seconds; for a blend tree, cycles × its blended length).
    pub time: f32,
    /// `time` over the motion's length: past 1 on a looping motion's second
    /// cycle; 0 while the length is unknown.
    pub normalized_time: f32,
    /// The motion's length in seconds (a blend tree's blended length); 0 when
    /// unknown.
    pub length: f32,
}

/// Which of the three state callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateEvent {
    Enter,
    Update,
    Exit,
}

/// One state callback the animation step queued for an animator's scripts.
#[derive(Clone, Debug, PartialEq)]
pub struct StateCallback {
    pub event: StateEvent,
    pub info: StateInfo,
}

/// A state last reported to the scripts: its identity, where its playhead
/// was, and the info last sent (what its `OnStateExit` carries).
#[derive(Clone, Debug, PartialEq)]
pub struct Reported {
    pub node: String,
    pub entry: u32,
    /// Its playhead as last read (playhead units, wrapped as the playback
    /// holds it), to measure the next step's travel from.
    pub raw: f32,
    /// Its playhead unwrapped since the state was entered (playhead units).
    pub total: f32,
    /// Whether it was the layer's current state (not the one fading out).
    pub current: bool,
    pub info: StateInfo,
}

/// The states last reported per layer (index 0 is the base layer); runtime
/// only, never saved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateTracker {
    pub layers: Vec<Vec<Reported>>,
}

impl StateTracker {
    /// Forget every reported state, firing nothing (`Animator.SetGraph`).
    pub fn clear(&mut self) {
        self.layers.clear();
    }
}
