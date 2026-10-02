//! The extra shader texture socket on the GPU (#400): a `dissolve` surface burns a
//! sphere away where its `mask` texture is below the runtime `amount`, while a
//! sphere whose material names no mask samples the white fallback — not the
//! checker a missing map shows — and stays whole. Skips with no adapter.

use std::cell::RefCell;

use super::params_tests::{halves, lua, sphere, RES};
use crate::render::Renderer;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

/// A dissolve surface shader and a mask PNG, unique to this run; removed on drop.
pub(crate) struct Dissolve {
    pub shader: String,
    pub mask: String,
}

impl Dissolve {
    /// With a mid-gray mask.
    fn bake() -> Self {
        Self::with_mask(
            "gray",
            &image::RgbaImage::from_pixel(4, 4, image::Rgba([128, 128, 128, 255])),
        )
    }

    /// With `mask` saved as this run's `<tag>` mask.
    pub(crate) fn with_mask(tag: &str, mask: &image::RgbaImage) -> Self {
        let shader = format!("test_dissolve_{tag}_{}", std::process::id());
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: shader.clone(),
            blocks: vec![BlockSel {
                id: "dissolve".into(),
                params: Default::default(),
            }],
        };
        bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).expect("bake succeeds");
        let path = crate::test_temp::dir().join(format!("{shader}_mask.png"));
        mask.save(&path).expect("mask written");
        let mask = path.to_string_lossy().into_owned();
        Self { shader, mask }
    }
}

impl Drop for Dissolve {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.shader));
        }
        let _ = std::fs::remove_file(&self.mask);
    }
}

/// A lit scene with two spheres drawn by the dissolve shader; only the left
/// (`masked`) one names the gray mask. Returns the scene and the two ids.
fn two_spheres(d: &Dissolve) -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let masked = sphere(&mut scene, -1.2, &d.shader);
    let plain = sphere(&mut scene, 1.2, &d.shader);
    let scene = RefCell::new(scene);
    // A long-bracket string: a Windows path's backslashes are not escapes there.
    let script = format!(
        r#"Material.SetShaderTexture({masked}, "mask", [==[{}]==])"#,
        d.mask
    );
    lua(&scene, &script);
    (scene, masked, plain)
}

/// The left half with the masked sphere moved out of view: what a hole shows.
fn backdrop(renderer: &mut Renderer, scene: &RefCell<Scene>, masked: u32) -> (u32, u32) {
    let moved = |x: f32| {
        scene
            .borrow_mut()
            .world
            .transform_mut(masked)
            .unwrap()
            .position
            .x = x
    };
    moved(100.0);
    let [bg, _] = halves(renderer, &scene.borrow());
    moved(-1.2);
    bg
}

#[test]
fn gpu_dissolve_burns_away_where_the_mask_is_below_amount_and_no_mask_is_white() {
    let d = Dissolve::bake();
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, masked, plain) = two_spheres(&d);
    let set = |id: u32, amount: f32| {
        let script = format!(r#"Material.SetShaderParam({id}, "dissolve.amount", {amount})"#);
        lua(&scene, &script)
    };

    let [l0, r0] = halves(&mut renderer, &scene.borrow());
    assert!(
        l0.1 > 0 && r0.1 > 0,
        "both whole at amount 0: {l0:?} {r0:?}"
    );
    let backdrop = backdrop(&mut renderer, &scene, masked);
    assert_ne!(backdrop, l0, "the sphere shows against the backdrop");

    // Gray (128, read raw as ≈ 0.5, #647) is above 0.4: the threshold, not a
    // blanket discard. Decoded as sRGB it would read ≈ 0.22 and burn here.
    set(masked, 0.4);
    let left = |r: &mut Renderer| halves(r, &scene.borrow())[0];
    assert_eq!(left(&mut renderer), l0, "above amount stays");

    // ...and below 0.6, so the masked sphere burns away; the white fallback is not.
    set(masked, 0.6);
    set(plain, 0.6);
    let [l1, r1] = halves(&mut renderer, &scene.borrow());
    assert_eq!(l1, backdrop, "the masked sphere burned away");
    assert_eq!(r1, r0, "no mask samples white, so nothing dissolves");

    set(masked, 0.0);
    assert_eq!(left(&mut renderer), l0, "whole again");
}
