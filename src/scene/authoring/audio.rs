//! src/scene/authoring/audio.rs — Shared audio-source-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `AudioSourceComponent` field by field: the `clip` path, per-source `volume`, the
//! playback flags (`looping` / `play_on_start` / `is_time_scaled`), and the spatial
//! fields (`spatial_blend` / `initial_distance` / `final_distance`).
//!
//! The editor's Audio Source card routes every field write through these (#287). The
//! Lua `Audio.*` namespace drives the live `AudioMaestro` voice (play / stop / live
//! volume), NOT the persisted component fields, so it shares no field write with this
//! card — the ops are the single source for the card alone, with unit tests standing
//! in for a convergence test. They are plain sets; the card's drag-range clamps are a
//! UI affordance applied to the widget local before the op is called.
//!
//! Allowed deps: components (the `AudioSourceComponent` data). Pure.

use crate::components::Entity;

/// Set the source's clip path.
pub fn set_clip(entity: &mut Entity, clip: String) {
    if let Some(a) = &mut entity.audio {
        a.clip = clip;
    }
}

/// Set the source's per-source linear volume.
pub fn set_volume(entity: &mut Entity, volume: f32) {
    if let Some(a) = &mut entity.audio {
        a.volume = volume;
    }
}

/// Set whether the source loops.
pub fn set_looping(entity: &mut Entity, looping: bool) {
    if let Some(a) = &mut entity.audio {
        a.looping = looping;
    }
}

/// Set whether the source plays automatically on Play.
pub fn set_play_on_start(entity: &mut Entity, play_on_start: bool) {
    if let Some(a) = &mut entity.audio {
        a.play_on_start = play_on_start;
    }
}

/// Set whether the source's rate follows `Time.timeScale`.
pub fn set_time_scaled(entity: &mut Entity, is_time_scaled: bool) {
    if let Some(a) = &mut entity.audio {
        a.is_time_scaled = is_time_scaled;
    }
}

/// Set the source's spatial blend (0 = 2D, 1 = fully 3D).
pub fn set_spatial_blend(entity: &mut Entity, spatial_blend: f32) {
    if let Some(a) = &mut entity.audio {
        a.spatial_blend = spatial_blend;
    }
}

/// Set the rolloff's near (full-volume) distance.
pub fn set_initial_distance(entity: &mut Entity, initial_distance: f32) {
    if let Some(a) = &mut entity.audio {
        a.initial_distance = initial_distance;
    }
}

/// Set the rolloff's far (silent) distance.
pub fn set_final_distance(entity: &mut Entity, final_distance: f32) {
    if let Some(a) = &mut entity.audio {
        a.final_distance = final_distance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::AudioSourceComponent;
    use crate::scene::Scene;

    fn scene_with_audio() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Speaker".to_string());
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.audio = Some(AudioSourceComponent::default());
        }
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_audio();
        let mut e = scene.get_entity_mut(id).unwrap();
        set_clip(&mut e, "sfx/hit.ogg".to_string());
        set_volume(&mut e, 0.5);
        set_looping(&mut e, true);
        set_play_on_start(&mut e, true);
        set_time_scaled(&mut e, false);
        set_spatial_blend(&mut e, 1.0);
        set_initial_distance(&mut e, 2.0);
        set_final_distance(&mut e, 20.0);
        let a = e.audio.as_ref().unwrap();
        assert_eq!(a.clip, "sfx/hit.ogg");
        assert_eq!(a.volume, 0.5);
        assert!(a.looping);
        assert!(a.play_on_start);
        assert!(!a.is_time_scaled);
        assert_eq!(a.spatial_blend, 1.0);
        assert_eq!(a.initial_distance, 2.0);
        assert_eq!(a.final_distance, 20.0);
    }
}
