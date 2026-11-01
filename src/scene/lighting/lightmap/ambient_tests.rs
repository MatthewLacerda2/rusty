//! The ambient a ray escaping the bake sees (#1647): an unoccluded texel bakes to the
//! ambient the forward shader gives an unbaked surface facing the same way, exactly
//! in `Color` mode and up to the L2 cut in `Gradient` (`AmbientRadiance::incoming`).

use glam::Vec3;

use super::input::BakeScene;
use super::rays::Rays;
use super::rng::Rng;
use super::trace::{TexelLight, Tracer};
use crate::core::color::luminance;
use crate::scene::AmbientRadiance;

/// The indirect light of one texel at the origin facing `normal`, in an empty scene.
pub(super) fn open_texel(sky: AmbientRadiance, normal: Vec3) -> Vec3 {
    open_light(sky, normal).indirect
}

/// Both parts of one texel at the origin facing `normal`, in an empty scene.
fn open_light(sky: AmbientRadiance, normal: Vec3) -> TexelLight {
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
    tracer.texel(Vec3::ZERO, normal, 40_000, 1, &mut rng)
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
fn a_horizon_glow_never_bakes_negative_light_or_direction() {
    // Dim poles, a bright equator: `incoming` rings to about -1.7 straight up, so
    // unclamped rays would take bounce light away (#810's invariant with it).
    let glow = AmbientRadiance::Trilight {
        sky: Vec3::splat(0.1),
        equator: Vec3::ONE,
        ground: Vec3::splat(0.1),
    };
    assert!(glow.incoming(Vec3::Y).max_element() < -1.0);
    for n in [Vec3::Y, Vec3::X, Vec3::NEG_Y] {
        let t = open_light(glow, n);
        assert!(
            t.indirect.min_element() > 0.0,
            "{n}: no negative channel, and the horizon reaches it: {}",
            t.indirect
        );
        let along = t.indirect_dir.dot(n);
        assert!(
            along > 0.0,
            "{n}: the light arrives from {}",
            t.indirect_dir
        );
        assert!(
            t.indirect_dir.length() <= luminance(t.indirect) + 1e-4,
            "{n}: |{}| > luminance {}",
            t.indirect_dir,
            luminance(t.indirect)
        );
    }
    // A floor's dominant direction points up, whatever the zenith rings to.
    let up = open_light(glow, Vec3::Y).indirect_dir.normalize();
    assert!(up.y > 0.9, "{up}");
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
