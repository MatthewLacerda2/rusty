//! The atlas follows the camera's light budget (#873): a light the per-camera cut
//! drops is never shaded, so it never wins tiles, and never counts as dropped from
//! the atlas either.

use glam::{Quat, Vec3};

use super::plan::{plan, AtlasPlan};
use crate::components::{LightComponent, TransformComponent};
use crate::render::clusters::{shadow_requests, ClusterGrid, LocalLight, MAX_VISIBLE_LIGHTS};
use crate::scene::{Camera, LightType};

const ASPECT: f32 = 16.0 / 9.0;
/// Lights past the camera budget: they lose its cut but, being intense, would
/// outrank every kept light by atlas importance (which ignores colour). Every
/// light is a point light (six tiles), so the kept ones alone overflow the atlas.
const OVER: usize = 16;

fn point(i: usize, color: Vec3, intensity: f32) -> LocalLight {
    let transform = TransformComponent {
        position: Vec3::new((i % 10) as f32 - 4.5, (i / 10) as f32 * 0.5 - 2.0, -8.0),
        rotation: Quat::IDENTITY,
        ..Default::default()
    };
    let light = LightComponent {
        light_type: LightType::Point,
        color,
        intensity,
        range: 4.0,
        inner_cone: 20.0,
        outer_cone: 30.0,
        cast_shadows: true,
        mode: Default::default(),
    };
    LocalLight::new(&transform, &light).expect("a local light")
}

/// `MAX_VISIBLE_LIGHTS` white lights that the camera keeps (light 0 asks for no
/// shadow), then [`OVER`] near-black, intense ones it drops.
fn lights() -> Vec<(LocalLight, bool)> {
    let kept = (0..MAX_VISIBLE_LIGHTS).map(|i| (point(i, Vec3::ONE, 1.0), i != 0));
    let dark = Vec3::splat(0.01);
    let over =
        (MAX_VISIBLE_LIGHTS..MAX_VISIBLE_LIGHTS + OVER).map(|i| (point(i, dark, 20.0), true));
    kept.chain(over).collect()
}

fn plan_for(camera: &Camera, lights: &[(LocalLight, bool)]) -> AtlasPlan {
    plan(
        lights,
        camera.build_view_projection(ASPECT),
        camera.position,
    )
}

fn owners(p: &AtlasPlan) -> Vec<usize> {
    p.shadows.iter().map(|s| s.0).collect()
}

#[test]
fn lights_the_camera_drops_get_no_tiles() {
    let camera = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let all = lights();
    let unbudgeted = owners(&plan_for(&camera, &all));
    assert!(
        unbudgeted.iter().any(|&i| i >= MAX_VISIBLE_LIGHTS),
        "the dropped lights must outrank the kept ones by atlas importance: {unbudgeted:?}"
    );
    let requests = shadow_requests(&ClusterGrid::new(&camera, ASPECT), &all);
    let p = plan_for(&camera, &requests);
    let got = owners(&p);
    assert!(!got.is_empty());
    assert!(got.iter().all(|&i| i < MAX_VISIBLE_LIGHTS), "{got:?}");
    assert!(
        !got.contains(&0),
        "a kept light that casts no shadow stays unshadowed"
    );
    assert!(p.dropped > 0, "the atlas fills up with kept lights alone");
    let shaded_casters = MAX_VISIBLE_LIGHTS as u32 - 1;
    assert_eq!(
        p.shadows.len() as u32 + p.dropped,
        shaded_casters,
        "every shaded caster is tiled or counted dropped; unshaded ones are neither"
    );
}

#[test]
fn requests_keep_the_lights_and_their_order() {
    let camera = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let all = lights();
    let requests = shadow_requests(&ClusterGrid::new(&camera, ASPECT), &all);
    assert_eq!(requests.len(), all.len());
    for (i, ((got, wants), (light, asks))) in requests.iter().zip(&all).enumerate() {
        assert_eq!(got.position, light.position, "light {i}");
        assert_eq!(*wants, *asks && i < MAX_VISIBLE_LIGHTS, "light {i}");
    }
}
