use serde_json::{json, Value};

use rusty_components::{ActiveMontage, AnimatorComponent, LayerState};

pub(crate) fn animator_value(a: &AnimatorComponent) -> Value {
    json!({
        "clip": a.base.current_clip,
        "time": a.base.time,
        "speed": a.speed,
        "playing": a.is_playing,
        "loop": a.base.loop_clip,
        "paused": a.freeze,
        "parameters": a.parameters,
        "graph": a.graph,
        "graph_enabled": a.graph_enabled,
        "node": a.base.current_node,
        "layers": a.layers.iter().map(layer_value).collect::<Vec<_>>(),
        "apply_root_motion": a.apply_root_motion,
        "root_bone": a.root_bone,
        "root_motion": {
            "position": a.root_motion.last.position.to_array(),
            "yaw": a.root_motion.last.yaw.to_degrees(),
        },
        "montages": a.montages.playing.iter().map(montage_value).collect::<Vec<_>>(),
    })
}

fn montage_value(m: &ActiveMontage) -> Value {
    json!({
        "slot": m.slot.name,
        "montage": m.montage.name,
        "section": m.section_name(),
        "time": m.time,
        "weight": m.weight(),
        "blending_out": m.out.is_some(),
    })
}
fn layer_value(l: &LayerState) -> Value {
    json!({ "name": l.name, "weight": l.weight, "node": l.playback.current_node })
}

super::fields::snapshot! {
    AnimatorComponent => animator_value,
}
