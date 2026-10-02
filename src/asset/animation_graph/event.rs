//! src/asset/animation_graph/event.rs — animation events (#459).
//!
//! A clip-timed marker: when the playhead crosses `time`, the animator's scripts
//! get `OnAnimationEvent(id, name)` — Unity's `AnimationEvent`, minus the payload
//! (a name is enough; Lua switches on it). glTF can't carry them and clips are
//! TRS tracks only, so they live on the graph: a clip node carries its clip's
//! events, and each blend-tree child carries its own. Detecting crossings and
//! dispatching is the runtime's job (`app::animation::events`).

use serde::{Deserialize, Serialize};

/// One marker: a time in the clip's own seconds and the name the callback gets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationEvent {
    /// Seconds into the clip (unscaled by any playback speed).
    pub time: f32,
    pub name: String,
}

/// Every problem with `events`, each message led by `label`.
pub(super) fn check_events(label: &str, events: &[AnimationEvent], problems: &mut Vec<String>) {
    for event in events {
        if event.name.is_empty() {
            problems.push(format!("{label} has an event with an empty name"));
        }
        if !event.time.is_finite() || event.time < 0.0 {
            problems.push(format!(
                "{label} event '{}' has a negative or non-finite time",
                event.name
            ));
        }
    }
}
