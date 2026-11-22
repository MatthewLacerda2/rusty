use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationEvent {
    /// Seconds into the clip (unscaled by any playback speed).
    pub time: f32,
    pub name: String,
}

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
