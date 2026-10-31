//! What covers SSR's reflection scales it (#1526): a translucent surface lets
//! through as much of it as of the floor, and an authored block that paints the
//! colour over (`hit_flash`) leaves none of it.

use glam::{Quat, Vec3};

use super::fixture::{frame, linear, paint, pixel, renderer, rgb, tuned, Surface};
use super::fixture::{MIRROR, REFLECTING};
use crate::components::{RenderMode, Tonemap};
use crate::render::Renderer;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

/// A black, rough cube: where the mirror floor sees it, SSR takes the sky away.
const BLACK: Surface = ([0.0; 3], 0.0, 1.0);

/// The SSR scene with a black cube, untonemapped so a frame reads back linear.
fn dark(on: bool) -> Scene {
    tuned(on, MIRROR, BLACK, |v| v.tonemapping.mode = Tonemap::None).0
}

/// How much SSR darkens [`REFLECTING`]'s pixel (off − on, per channel, linear).
fn darkening(r: &mut Renderer, scene: impl Fn(bool) -> Scene) -> [f32; 3] {
    let at = pixel(REFLECTING);
    let (on, off) = (
        linear(&frame(r, &scene(true)), at),
        linear(&frame(r, &scene(false)), at),
    );
    [0, 1, 2].map(|c| off[c] - on[c])
}

#[test]
fn gpu_glass_over_a_mirror_lets_through_as_much_of_the_reflection_as_of_the_floor() {
    let Some(mut r) = renderer() else { return };
    let alpha = 0.8;
    let glazed = |on| {
        let mut scene = dark(on);
        // A plane, one face: a box's back face would cover the floor twice.
        let pane = create_entity(&mut scene, "Glass", Some(Primitive::Plane));
        let mut t = scene.world.transform_mut(pane).unwrap();
        (t.position, t.scale) = (Vec3::new(0.0, 1.5, -1.0), Vec3::splat(0.3));
        t.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        drop(t);
        paint(&mut scene, pane, "glass", ([0.5; 3], 0.0, 1.0));
        let glass = scene.materials.get_mut("glass").unwrap();
        (glass.render_mode, glass.alpha) = (RenderMode::Transparent, alpha);
        scene
    };
    let bare = darkening(&mut r, dark);
    let glass = darkening(&mut r, glazed);
    eprintln!("[ssr] glass: SSR darkens the bare floor by {bare:?}, under glass {glass:?}");
    assert!(bare[1] > 0.2, "SSR swaps the sky for the black cube");
    // The glass lets 1 − α of the floor through, and as much of SSR's change. A
    // reflection swapped after the glass took the whole sky away from under it.
    for c in 0..3 {
        let expected = (1.0 - alpha) * bare[c];
        assert!(
            (glass[c] - expected).abs() <= 0.03,
            "channel {c}: {glass:?}"
        );
    }
}

/// A baked `hit_flash` surface shader, unique to this run; removed on drop.
struct HitFlash(String);

impl HitFlash {
    fn bake() -> Self {
        let name = format!("test_ssr_hit_flash_{}", std::process::id());
        let block = BlockSel {
            id: "hit_flash".into(),
            params: Default::default(),
        };
        let blocks = vec![block];
        let (pass, name) = (PassKind::Surface, name.clone());
        let recipe = ShaderRecipe { pass, name, blocks };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
        Self(recipe.name)
    }
}

impl Drop for HitFlash {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

#[test]
fn gpu_a_full_hit_flash_on_a_mirror_shows_no_reflection() {
    let Some(mut r) = renderer() else { return };
    let shader = HitFlash::bake();
    let flashed = |on| {
        let mut scene = dark(on);
        let floor = scene.materials.get_mut("floor").unwrap();
        floor.shader = Some(shader.0.clone());
        let params = &mut floor.shader_params;
        params.insert("hit_flash.color".into(), vec![0.2, 0.4, 0.6]);
        params.insert("hit_flash.amount".into(), vec![1.0]);
        scene
    };
    let at = pixel(REFLECTING);
    let (on, off) = (
        rgb(&frame(&mut r, &flashed(true)), at),
        rgb(&frame(&mut r, &flashed(false)), at),
    );
    eprintln!("[ssr] hit_flash 1: on {on:?}, off {off:?}");
    // The flash paints the floor flat: no reflection is left in it for SSR to swap.
    assert!(off[2] > off[0] + 20, "the floor shows the flash colour");
    assert!(
        (0..3).all(|c| (on[c] - off[c]).abs() <= 1),
        "{on:?} vs {off:?}"
    );
}
