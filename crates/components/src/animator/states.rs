#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeStates {
    pub previous_node: Option<String>,
    /// Counts node entries, so re-entering the playing node reads as a new state.
    pub entry: u32,
    pub previous_entry: u32,
    pub travel: f32,
}

impl NodeStates {
    pub fn entered(&mut self, left: Option<String>, fading: bool) {
        self.previous_node = left.filter(|_| fading);
        self.previous_entry = self.entry;
        self.entry = self.entry.wrapping_add(1);
    }
}

/// One graph state as the scripts see it: Unity's `AnimatorStateInfo`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateInfo {
    pub layer: usize,
    /// The layer's name: `"Base Layer"` for layer 0, as in Unity.
    pub layer_name: String,
    pub state: String,
    pub tag: Option<String>,
    /// Seconds of the state's motion played since it was entered (the clip's
    /// own seconds; for a blend tree, cycles × its blended length).
    pub time: f32,
    pub normalized_time: f32,
    pub length: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateEvent {
    Enter,
    Update,
    Exit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StateCallback {
    pub event: StateEvent,
    pub info: StateInfo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reported {
    pub node: String,
    pub entry: u32,
    pub raw: f32,
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
    pub fn clear(&mut self) {
        self.layers.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.layers.iter().all(Vec::is_empty)
    }
}
