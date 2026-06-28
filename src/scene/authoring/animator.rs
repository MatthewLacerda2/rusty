//! src/scene/authoring/animator.rs — Shared animator-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `AnimatorComponent` field by field: the `current_clip` name, the playback
//! `speed`, and the `is_playing` / `freeze` flags.
//!
//! The editor's Animator card routes every field write through these (#287). The Lua
//! `Animator.*` surface is mostly *operations* over the component's own state machine
//! (`Play` / `Crossfade` call `AnimatorComponent::play`/`crossfade`), which are not
//! field writes; only `Animator.Stop` is a plain field write (`is_playing = false`)
//! and it routes through [`set_playing`] here, so the card's "Is Playing" toggle and
//! the binding share that one write.
//!
//! Allowed deps: components (the `AnimatorComponent` data). Pure.

use crate::components::Entity;

/// Set the animator's current clip name.
pub fn set_clip(entity: &mut Entity, clip: String) {
    if let Some(a) = &mut entity.animator {
        a.current_clip = clip;
    }
}

/// Set the animator's playback speed multiplier.
pub fn set_speed(entity: &mut Entity, speed: f32) {
    if let Some(a) = &mut entity.animator {
        a.speed = speed;
    }
}

/// Set the animator's `is_playing` flag.
pub fn set_playing(entity: &mut Entity, is_playing: bool) {
    if let Some(a) = &mut entity.animator {
        a.is_playing = is_playing;
    }
}

/// Set the animator's `freeze` (editor/debug hold) flag.
pub fn set_freeze(entity: &mut Entity, freeze: bool) {
    if let Some(a) = &mut entity.animator {
        a.freeze = freeze;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::AnimatorComponent;
    use crate::scene::Scene;

    fn scene_with_animator() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Rig".to_string());
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.animator = Some(AnimatorComponent::default());
        }
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_animator();
        let mut e = scene.get_entity_mut(id).unwrap();
        set_clip(&mut e, "Run".to_string());
        set_speed(&mut e, 2.0);
        set_playing(&mut e, true);
        set_freeze(&mut e, true);
        let a = e.animator.as_ref().unwrap();
        assert_eq!(a.current_clip, "Run");
        assert_eq!(a.speed, 2.0);
        assert!(a.is_playing);
        assert!(a.freeze);
    }
}
