//! src/api/snapshot/animator.rs — the animator's snapshot field.
//!
//! Its playback state, graph binding, layers, root motion and the montages
//! playing in its slots (#1894). Split out of `components.rs` to keep that file
//! under the size cap; the keys are unchanged (`docs/api/Debug.md`).

use serde_json::{json, Value};

use crate::components::{ActiveMontage, AnimatorComponent, LayerState};

/// Animator playback state plus the typed graph parameters (#314) and the
/// animation-graph binding (#316), so a script can read back what `Animator.Set*`
/// wrote and which graph state the evaluator holds active. Each parameter
/// serializes externally tagged, e.g. `{"Bool": true}` / `{"Trigger": false}`; the
/// `BTreeMap` keeps the keys name-sorted. `layers` lists the extra layers (#457),
/// `montages` what plays in the slots (#1894).
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

/// One playing montage (#1894): runtime only, never saved.
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
