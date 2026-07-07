//! src/scene/authoring/animator.rs — Shared animator-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `AnimatorComponent` field by field: the `current_clip` name, the playback
//! `speed`, and the `is_playing` / `freeze` / `loop_clip` flags.
//!
//! The editor's Animator card routes every field write through these (#287). The Lua
//! `Animator.*` surface is mostly *operations* over the component's own state machine
//! (`Play` / `Crossfade` call `AnimatorComponent::play`/`crossfade`), which are not
//! field writes; the plain field writes — `Animator.Stop` (`is_playing = false`),
//! `Animator.Pause`/`Resume` (`freeze`, #313) and `Animator.SetLooping`
//! (`loop_clip`, #313) — route through [`set_playing`] / [`set_freeze`] /
//! [`set_looping`] here, so the card's toggles and the bindings share each write.
//!
//! Allowed deps: components (the `AnimatorComponent` data). Pure.

use crate::components::AnimatorComponent;

/// Set the animator's current clip name.
pub fn set_clip(a: &mut AnimatorComponent, clip: String) {
    a.current_clip = clip;
}

/// Set the animator's playback speed multiplier.
pub fn set_speed(a: &mut AnimatorComponent, speed: f32) {
    a.speed = speed;
}

/// Set the animator's `is_playing` flag.
pub fn set_playing(a: &mut AnimatorComponent, is_playing: bool) {
    a.is_playing = is_playing;
}

/// Set the animator's `freeze` (pause hold) flag — `Animator.Pause`/`Resume` and
/// the card's Freeze toggle.
pub fn set_freeze(a: &mut AnimatorComponent, freeze: bool) {
    a.freeze = freeze;
}

/// Set the animator's `loop_clip` flag (wrap the playhead at the clip's end).
pub fn set_looping(a: &mut AnimatorComponent, loop_clip: bool) {
    a.loop_clip = loop_clip;
}

/// Set (or clear, with `None`) the animator's `AnimationGraph` asset path (#316).
/// Resets the active-node state so the evaluator re-binds: its next step seeds the
/// new graph's declared defaults and enters its entry node.
pub fn set_graph(a: &mut AnimatorComponent, graph: Option<String>) {
    a.graph = graph;
    a.current_node = None;
    a.node_speed = 1.0;
}

/// Enable/disable graph auto-evaluation (#316). Disabled, the graph is inert and
/// the animator stays under direct `Play`/`Crossfade` control.
pub fn set_graph_enabled(a: &mut AnimatorComponent, enabled: bool) {
    a.graph_enabled = enabled;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::AnimatorComponent;
    use crate::scene::Scene;

    fn scene_with_animator() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Rig".to_string());
        scene.world.set_animator(id, Some(AnimatorComponent::default()));
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_animator();
        let mut e = scene.world.animator_mut(id).unwrap();
        set_clip(&mut e, "Run".to_string());
        set_speed(&mut e, 2.0);
        set_playing(&mut e, true);
        set_freeze(&mut e, true);
        set_looping(&mut e, true);
        set_graph(&mut e, Some("guard.animgraph".to_string()));
        set_graph_enabled(&mut e, false);
        let a = &*e;
        assert_eq!(a.current_clip, "Run");
        assert_eq!(a.speed, 2.0);
        assert!(a.is_playing);
        assert!(a.freeze);
        assert!(a.loop_clip);
        assert_eq!(a.graph.as_deref(), Some("guard.animgraph"));
        assert!(!a.graph_enabled);
    }

    #[test]
    fn set_graph_resets_the_active_node_state() {
        let (mut scene, id) = scene_with_animator();
        let mut e = scene.world.animator_mut(id).unwrap();
        {
            let a = (&mut *e);
            a.current_node = Some("Run".to_string());
            a.node_speed = 2.0;
        }
        set_graph(&mut e, None);
        let a = &*e;
        assert_eq!(a.graph, None);
        assert_eq!(a.current_node, None, "the stale active node is dropped");
        assert_eq!(a.node_speed, 1.0, "the node speed override is dropped");
    }
}
