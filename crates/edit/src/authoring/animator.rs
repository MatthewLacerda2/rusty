use rusty_components::AnimatorComponent;
use rusty_scene::rules::sanitize::finite_or;

pub fn set_clip(a: &mut AnimatorComponent, clip: String) {
    a.base.current_clip = clip;
    a.base.current_tree = None;
    a.base.ended = false;
}

/// A non-finite speed keeps the old one, or it would poison the playhead; the guard sits here
/// because the inspector's field parses `inf`.
pub fn set_speed(a: &mut AnimatorComponent, speed: f32) {
    a.speed = finite_or(speed, a.speed);
}

pub fn set_playing(a: &mut AnimatorComponent, is_playing: bool) {
    a.is_playing = is_playing;
}

pub fn set_freeze(a: &mut AnimatorComponent, freeze: bool) {
    a.freeze = freeze;
}

pub fn set_looping(a: &mut AnimatorComponent, loop_clip: bool) {
    a.base.loop_clip = loop_clip;
}

pub fn set_graph(a: &mut AnimatorComponent, graph: Option<String>) {
    a.graph = graph;
    a.base.current_node = None;
    a.base.states = Default::default();
    a.base.node_speed = 1.0;
    // The old graph's sync group would otherwise match a new node's by name.
    a.base.sync = Default::default();
    a.layers.clear();
    // A montage ran against the old graph's slots: it goes, firing nothing.
    a.montages.clear();
    a.states.clear();
}

pub fn set_graph_enabled(a: &mut AnimatorComponent, enabled: bool) {
    a.graph_enabled = enabled;
}

pub fn set_apply_root_motion(a: &mut AnimatorComponent, apply: bool) {
    a.apply_root_motion = apply;
}

pub fn set_root_bone(a: &mut AnimatorComponent, bone: String) {
    a.root_bone = bone;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_components::AnimatorComponent;
    use rusty_scene::Scene;

    fn scene_with_animator() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Rig".to_string());
        scene
            .world
            .set_animator(id, Some(AnimatorComponent::default()));
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
        set_apply_root_motion(&mut e, true);
        set_root_bone(&mut e, "Hips".to_string());
        let a = &*e;
        assert_eq!(a.base.current_clip, "Run");
        assert_eq!(a.speed, 2.0);
        assert!(a.is_playing);
        assert!(a.freeze);
        assert!(a.base.loop_clip);
        assert_eq!(a.graph.as_deref(), Some("guard.animgraph"));
        assert!(!a.graph_enabled);
        assert!(a.apply_root_motion);
        assert_eq!(a.root_bone, "Hips");
    }

    #[test]
    fn set_speed_keeps_the_old_speed_on_a_non_finite_value() {
        let (mut scene, id) = scene_with_animator();
        let mut e = scene.world.animator_mut(id).unwrap();
        set_speed(&mut e, -0.5);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            set_speed(&mut e, bad);
            assert_eq!(e.speed, -0.5, "{bad} is refused");
        }
    }

    #[test]
    fn set_graph_resets_the_active_node_state() {
        let (mut scene, id) = scene_with_animator();
        let mut e = scene.world.animator_mut(id).unwrap();
        {
            let a = &mut *e;
            a.base.current_node = Some("Run".to_string());
            a.base.node_speed = 2.0;
        }
        set_graph(&mut e, None);
        let a = &*e;
        assert_eq!(a.graph, None);
        assert_eq!(
            a.base.current_node, None,
            "the stale active node is dropped"
        );
        assert_eq!(a.base.node_speed, 1.0, "the node speed override is dropped");
    }
}
