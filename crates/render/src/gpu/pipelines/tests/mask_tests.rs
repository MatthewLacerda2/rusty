use std::cell::RefCell;
use std::rc::Rc;

use super::params_tests::{halves, set_param, set_texture, sphere, RES};
use crate::Renderer;
use rusty_scene::create::{create_live_light, Primitive};
use rusty_scene::Scene;
use rusty_shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use rusty_shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

pub(crate) struct Dissolve {
    pub shader: String,
    pub mask: String,
}

impl Dissolve {
    fn bake() -> Self {
        Self::with_mask(
            "gray",
            &image::RgbaImage::from_pixel(4, 4, image::Rgba([128, 128, 128, 255])),
        )
    }

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
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
        let path = rusty_test_support::temp::dir().join(format!("{shader}_mask.png"));
        mask.save(&path).expect("mask written");
        let mask = path.to_string_lossy().into_owned();
        Self { shader, mask }
    }
}

impl Drop for Dissolve {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.shader));
        }
        let _ = std::fs::remove_file(&self.mask);
    }
}

fn two_spheres(d: &Dissolve) -> (Rc<RefCell<Scene>>, u32, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let masked = sphere(&mut scene, -1.2, &d.shader);
    let plain = sphere(&mut scene, 1.2, &d.shader);
    let scene = Rc::new(RefCell::new(scene));
    set_texture(&scene, masked, "mask", &d.mask);
    (scene, masked, plain)
}

fn backdrop(renderer: &mut Renderer, scene: &Rc<RefCell<Scene>>, masked: u32) -> (u32, u32) {
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
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (scene, masked, plain) = two_spheres(&d);
    let set = |id: u32, amount: f32| set_param(&scene, id, "dissolve.amount", amount);

    let [l0, r0] = halves(&mut renderer, &scene.borrow());
    assert!(
        l0.1 > 0 && r0.1 > 0,
        "both whole at amount 0: {l0:?} {r0:?}"
    );
    let backdrop = backdrop(&mut renderer, &scene, masked);
    assert_ne!(backdrop, l0, "the sphere shows against the backdrop");

    set(masked, 0.4);
    let left = |r: &mut Renderer| halves(r, &scene.borrow())[0];
    assert_eq!(left(&mut renderer), l0, "above amount stays");

    set(masked, 0.6);
    set(plain, 0.6);
    let [l1, r1] = halves(&mut renderer, &scene.borrow());
    assert_eq!(l1, backdrop, "the masked sphere burned away");
    assert_eq!(r1, r0, "no mask samples white, so nothing dissolves");

    set(masked, 0.0);
    assert_eq!(left(&mut renderer), l0, "whole again");
}
