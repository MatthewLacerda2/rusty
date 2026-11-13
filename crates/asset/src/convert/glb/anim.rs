//! Serialise a [`Document`](super::Document)'s animations: one sampler per channel,
//! all `LINEAR`, times and values each in their own accessor.

use super::bin::Bin;
use super::{Animation, Keys};
use serde_json::{json, Value};

pub(super) fn animation(animation: &Animation, bin: &mut Bin) -> Value {
    let mut samplers = Vec::new();
    let mut channels = Vec::new();
    for channel in &animation.channels {
        let input = bin.times(&channel.times);
        let (path, output) = match &channel.keys {
            Keys::Translation(v) => ("translation", bin.data(v, "VEC3")),
            Keys::Rotation(v) => ("rotation", bin.data(v, "VEC4")),
            Keys::Scale(v) => ("scale", bin.data(v, "VEC3")),
        };
        channels.push(json!({
            "sampler": samplers.len(),
            "target": {"node": channel.node, "path": path},
        }));
        samplers.push(json!({"input": input, "output": output, "interpolation": "LINEAR"}));
    }
    json!({"name": animation.name, "channels": channels, "samplers": samplers})
}
