use glam::Vec3;

use rusty_scene::rules::sanitize::{
    clamped, finite3_or, finite_or, non_negative, optional_color_or,
};
use rusty_scene::{AmbientMode, AmbientSettings, Scene, SkySettings};

pub fn set_skybox(scene: &mut Scene, path: &str) {
    scene.skybox_path = path.to_string();
}

/// Set the ambient colour (linear RGB), each channel clamped to `≥ 0`. A colour
/// with a NaN or infinite channel is refused whole and changes nothing.
pub fn set_ambient_color(scene: &mut Scene, color: Vec3) {
    scene.ambient_color = finite3_or(color, scene.ambient_color).max(Vec3::ZERO);
}

/// Set the ambient intensity, clamped to `≥ 0`.
pub fn set_ambient_intensity(scene: &mut Scene, intensity: f32) {
    scene.ambient_intensity = non_negative(intensity);
}

pub fn set_ambient_mode(ambient: &mut AmbientSettings, mode: AmbientMode) {
    ambient.mode = mode;
}

/// Set the gradient's sky colour (linear RGB, up-facing surfaces), each channel
/// clamped to `≥ 0`. A NaN or infinite channel refuses the colour whole.
pub fn set_ambient_sky_color(ambient: &mut AmbientSettings, color: Vec3) {
    ambient.sky = finite3_or(color, ambient.sky).max(Vec3::ZERO);
}

pub fn set_ambient_equator_color(ambient: &mut AmbientSettings, color: Vec3) {
    ambient.equator = finite3_or(color, ambient.equator).max(Vec3::ZERO);
}

pub fn set_ambient_ground_color(ambient: &mut AmbientSettings, color: Vec3) {
    ambient.ground = finite3_or(color, ambient.ground).max(Vec3::ZERO);
}

pub fn held_ambient(ambient: AmbientSettings) -> AmbientSettings {
    let mut held = AmbientSettings::default();
    set_ambient_mode(&mut held, ambient.mode);
    set_ambient_sky_color(&mut held, ambient.sky);
    set_ambient_equator_color(&mut held, ambient.equator);
    set_ambient_ground_color(&mut held, ambient.ground);
    held
}

pub fn set_sky_zenith(sky: &mut SkySettings, color: Option<Vec3>) {
    sky.zenith = optional_color_or(color, sky.zenith);
}

pub fn set_sky_horizon(sky: &mut SkySettings, color: Option<Vec3>) {
    sky.horizon = optional_color_or(color, sky.horizon);
}

pub fn set_sky_ground(sky: &mut SkySettings, color: Option<Vec3>) {
    sky.ground = optional_color_or(color, sky.ground);
}

/// Set the cloud coverage, clamped to `0..=1` (`0`: a clear sky). A NaN or
/// infinite value changes nothing.
pub fn set_cloud_coverage(sky: &mut SkySettings, coverage: f32) {
    let v = finite_or(coverage, sky.cloud_coverage);
    sky.cloud_coverage = clamped(v, 0.0, 1.0);
}

pub fn set_cloud_opacity(sky: &mut SkySettings, opacity: f32) {
    let v = finite_or(opacity, sky.cloud_opacity);
    sky.cloud_opacity = clamped(v, 0.0, 1.0);
}

pub fn held_sky(sky: SkySettings) -> SkySettings {
    let mut held = SkySettings::default();
    set_sky_zenith(&mut held, sky.zenith);
    set_sky_horizon(&mut held, sky.horizon);
    set_sky_ground(&mut held, sky.ground);
    set_cloud_coverage(&mut held, sky.cloud_coverage);
    set_cloud_opacity(&mut held, sky.cloud_opacity);
    held
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_with_clamps() {
        let mut scene = Scene::new();
        set_skybox(&mut scene, "sky/day.hdr");
        set_ambient_color(&mut scene, Vec3::new(-1.0, 0.5, 2.0));
        set_ambient_color(&mut scene, Vec3::new(1.0, f32::NAN, 1.0));
        set_ambient_intensity(&mut scene, -0.5);
        assert_eq!(scene.skybox_path, "sky/day.hdr");
        assert_eq!(
            scene.ambient_color,
            Vec3::new(0.0, 0.5, 2.0),
            "NaN keeps it"
        );
        assert_eq!(scene.ambient_intensity, 0.0);
        set_ambient_intensity(&mut scene, f32::INFINITY);
        assert_eq!(scene.ambient_intensity, 0.0, "infinite is refused");
        set_skybox(&mut scene, "");
        assert!(scene.skybox_path.is_empty(), "empty is the procedural sky");
    }

    #[test]
    fn sky_ops_clamp_and_refuse_non_finite() {
        let mut scene = Scene::new();
        let sky = &mut scene.sky;
        set_cloud_coverage(sky, 2.0);
        set_cloud_opacity(sky, -1.0);
        assert_eq!((sky.cloud_coverage, sky.cloud_opacity), (1.0, 0.0));
        set_cloud_coverage(sky, f32::NAN);
        set_cloud_opacity(sky, f32::INFINITY);
        assert_eq!((sky.cloud_coverage, sky.cloud_opacity), (1.0, 0.0));
        set_sky_zenith(sky, Some(Vec3::new(-1.0, 0.5, 2.0)));
        set_sky_zenith(sky, Some(Vec3::new(f32::NAN, 0.0, 0.0)));
        assert_eq!(sky.zenith, Some(Vec3::new(0.0, 0.5, 2.0)), "NaN keeps it");
        set_sky_ground(sky, Some(Vec3::ONE));
        set_sky_horizon(sky, Some(Vec3::ONE));
        set_sky_ground(sky, None);
        assert_eq!((sky.ground, sky.horizon), (None, Some(Vec3::ONE)));
    }

    #[test]
    fn ambient_ops_clamp_and_refuse_non_finite() {
        let mut ambient = AmbientSettings::default();
        set_ambient_mode(&mut ambient, AmbientMode::Gradient);
        set_ambient_sky_color(&mut ambient, Vec3::new(-1.0, 0.5, 2.0));
        set_ambient_equator_color(&mut ambient, Vec3::new(f32::NAN, 0.0, 0.0));
        set_ambient_ground_color(&mut ambient, Vec3::new(0.0, f32::INFINITY, 0.0));
        let default = AmbientSettings::default();
        assert_eq!(ambient.mode, AmbientMode::Gradient);
        assert_eq!(ambient.sky, Vec3::new(0.0, 0.5, 2.0));
        assert_eq!(ambient.equator, default.equator, "NaN keeps it");
        assert_eq!(ambient.ground, default.ground, "infinite keeps it");
        let held = held_ambient(AmbientSettings {
            ground: Vec3::splat(-3.0),
            equator: Vec3::splat(f32::NAN),
            ..default
        });
        assert_eq!((held.ground, held.equator), (Vec3::ZERO, default.equator));
        assert!(held_ambient(default).is_default());
    }

    #[test]
    fn a_loaded_sky_is_held() {
        let sky = held_sky(SkySettings {
            cloud_coverage: 7.0,
            ground: Some(Vec3::splat(-3.0)),
            ..SkySettings::default()
        });
        assert_eq!(sky.cloud_coverage, 1.0);
        assert_eq!(sky.ground, Some(Vec3::ZERO));
        assert!(held_sky(SkySettings::default()).is_default());
    }
}
