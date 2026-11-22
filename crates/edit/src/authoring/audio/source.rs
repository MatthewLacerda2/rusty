use rusty_components::AudioSourceComponent;
use rusty_scene::rules::sanitize::{non_negative, unit};

pub fn set_clip(a: &mut AudioSourceComponent, clip: String) {
    a.clip = clip;
}

pub fn set_volume(a: &mut AudioSourceComponent, volume: f32) {
    a.volume = unit(volume);
}

pub fn set_looping(a: &mut AudioSourceComponent, looping: bool) {
    a.looping = looping;
}

pub fn set_play_on_start(a: &mut AudioSourceComponent, play_on_start: bool) {
    a.play_on_start = play_on_start;
}

pub fn set_time_scaled(a: &mut AudioSourceComponent, is_time_scaled: bool) {
    a.is_time_scaled = is_time_scaled;
}

pub fn set_spatial_blend(a: &mut AudioSourceComponent, spatial_blend: f32) {
    a.spatial_blend = unit(spatial_blend);
}

pub fn set_initial_distance(a: &mut AudioSourceComponent, initial_distance: f32) {
    a.initial_distance = non_negative(initial_distance);
}

pub fn set_final_distance(a: &mut AudioSourceComponent, final_distance: f32) {
    a.final_distance = non_negative(final_distance);
}

pub fn set_output_group(a: &mut AudioSourceComponent, group: String) {
    a.output_group = group;
}

pub fn set_occlusion_enabled(a: &mut AudioSourceComponent, on: bool) {
    a.occlusion_enabled = on;
}

pub fn set_doppler_level(a: &mut AudioSourceComponent, level: f32) {
    a.doppler_level = non_negative(level);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_components::AudioSourceComponent;
    use rusty_scene::Scene;

    fn scene_with_audio() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Speaker".to_string());
        scene
            .world
            .set_audio(id, Some(AudioSourceComponent::default()));
        (scene, id)
    }

    #[test]
    fn ops_write_through() {
        let (mut scene, id) = scene_with_audio();
        let mut e = scene.world.audio_mut(id).unwrap();
        set_clip(&mut e, "sfx/hit.ogg".to_string());
        set_volume(&mut e, 0.5);
        set_looping(&mut e, true);
        set_play_on_start(&mut e, true);
        set_time_scaled(&mut e, false);
        set_spatial_blend(&mut e, 1.0);
        set_initial_distance(&mut e, 2.0);
        set_final_distance(&mut e, 20.0);
        set_output_group(&mut e, "Music".to_string());
        set_occlusion_enabled(&mut e, false);
        set_doppler_level(&mut e, 2.5);
        let a = &*e;
        assert_eq!(a.clip, "sfx/hit.ogg");
        assert_eq!(a.volume, 0.5);
        assert!(a.looping);
        assert!(a.play_on_start);
        assert!(!a.is_time_scaled);
        assert_eq!(a.spatial_blend, 1.0);
        assert_eq!(a.initial_distance, 2.0);
        assert_eq!(a.final_distance, 20.0);
        assert_eq!(a.output_group, "Music");
        assert!(!a.occlusion_enabled);
        assert_eq!(a.doppler_level, 2.5);
    }
}
