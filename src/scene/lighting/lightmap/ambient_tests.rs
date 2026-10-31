//! The ambient a ray escaping the bake sees (#1647): an unoccluded texel bakes to the
//! ambient the forward shader gives an unbaked surface facing the same way, exactly
//! in `Color` mode and up to the L2 cut in `Gradient` (`AmbientRadiance::incoming`).

use glam::Vec3;

use super::input::BakeScene;
use super::rays::Rays;
use super::rng::Rng;
use super::trace::Tracer;
use crate::scene::AmbientRadiance;

/// The indirect light of one texel at the origin facing `normal`, in an empty scene.
fn open_texel(sky: AmbientRadiance, normal: Vec3) -> Vec3 {
    let scene = BakeScene {
        sky,
        ..Default::default()
    };
    let rays = Rays::build(Vec::new());
    let tracer = Tracer {
        scene: &scene,
        rays: &rays,
    };
    let mut rng = Rng::new(7, 0, 0);
    tracer
        .texel(Vec3::ZERO, normal, 40_000, 1, &mut rng)
        .indirect
}

#[test]
fn an_open_texel_bakes_to_the_unbaked_ambient_in_both_modes() {
    let color = AmbientRadiance::Hemisphere(Vec3::new(0.4, 0.8, 1.2));
    let gradient = AmbientRadiance::Trilight {
        sky: Vec3::new(0.1, 0.3, 1.2),
        equator: Vec3::splat(0.4),
        ground: Vec3::new(1.2, 0.5, 0.1),
    };
    let normals = [
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::X,
        Vec3::new(1.0, 1.0, 0.0).normalize(),
        Vec3::new(0.0, -2.0, 1.0).normalize(),
    ];
    for n in normals {
        let baked = open_texel(color, n);
        // Monte Carlo noise only: 40k cosine samples hold the mean to about 1%.
        let want = color.at(n);
        assert!(
            (baked - want).abs().max_element() < 0.02,
            "Color {n}: {baked} vs {want}"
        );
        let baked = open_texel(gradient, n);
        let want = gradient.at_l2(n);
        assert!(
            (baked - want).abs().max_element() < 0.02,
            "Gradient {n}: {baked} vs {want}"
        );
    }
}

#[test]
fn the_gather_takes_the_gradient_and_an_ambient_light_overrides_it() {
    use crate::components::{LightComponent, LightType};
    use crate::scene::{AmbientMode, Scene};
    let mut scene = Scene::new();
    scene.ambient.mode = AmbientMode::Gradient;
    scene.ambient.sky = Vec3::new(0.1, 0.3, 1.2);
    scene.ambient_intensity = 2.0;
    let sky = BakeScene::gather(&scene, &|_| None).sky;
    let a = scene.ambient;
    let want = AmbientRadiance::Trilight {
        sky: a.sky * 2.0,
        equator: a.equator * 2.0,
        ground: a.ground * 2.0,
    };
    assert_eq!(sky, want);
    let id = scene.add_entity("Ambient".into());
    let light = LightComponent {
        light_type: LightType::Ambient,
        color: Vec3::new(1.0, 0.0, 0.0),
        intensity: 0.5,
        ..crate::scene::authoring::default_light()
    };
    scene.world.set_light(id, Some(light));
    let sky = BakeScene::gather(&scene, &|_| None).sky;
    assert_eq!(sky, AmbientRadiance::Hemisphere(Vec3::new(0.5, 0.0, 0.0)));
}
