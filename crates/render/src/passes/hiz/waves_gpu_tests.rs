//! A material that moves its vertices is never culled by its bounds (#1963): a
//! `vertex_waves` plane's box is zero metres tall, so behind a low rim the box is
//! hidden while the swells' crests rise above it, and the surface must still draw.

use glam::Vec3;

use super::gpu_tests::{drawn, offscreen, shot, H, W};
use rusty_scene::authoring::rendering::mesh::set_subdivisions;
use rusty_scene::authoring::{create_entity, material, Primitive};
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};
use rusty_shadergen::recipe::{BlockSel, ParamValue, PassKind, ShaderRecipe};
use rusty_shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

/// The variant's baked files, removed when the test ends, pass or fail.
struct Baked(String);

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// A rim 2 m tall at z = 0 and, 4 m behind it, a 2 m water plane at y = 0 drawn
/// with `water` (its swells rise metres over the rim), or none when `None`. The
/// plane is narrow, so its flat box is tested at a fine pyramid level, all rim.
fn pool(water: Option<&str>) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let rim = (Vec3::new(-8.0, -2.0, -0.2), Vec3::new(8.0, 2.0, 0.2));
    block(&mut scene, "Rim", rim.0, rim.1);
    if let Some(shader) = water {
        let id = create_entity(&mut scene, "Water", Some(Primitive::Plane));
        set_subdivisions(&mut scene.world.mesh_mut(id).unwrap(), 32).unwrap();
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (Vec3::new(0.0, 0.0, -4.0), Vec3::new(0.2, 1.0, 0.2)); // a 10 m Plane, made 2 m
        drop(t);
        let key = material::ensure_material_key(&mut scene, id).unwrap();
        material::set_shader(&mut scene.materials, &key, shader.to_string());
    }
    scene
}

#[test]
fn gpu_a_waved_plane_behind_a_rim_still_draws_its_crests() {
    let name = format!("test_hiz_waves_{}", std::process::id());
    let _baked = Baked(name.clone());
    let params = [("amplitude".to_string(), ParamValue::Scalar(6.0))];
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: name.clone(),
        blocks: vec![BlockSel {
            id: "vertex_waves".into(),
            params: params.into_iter().collect(),
        }],
    };
    bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bakes");
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    // Low, well under the rim's top: the flat box behind it is hidden.
    let cam = Camera::new(Vec3::new(0.0, 0.5, 6.0), -90.0, 0.0);
    let (water, dry) = (pool(Some(&name)), pool(None));
    let mut view = offscreen(&r);
    let (empty, _) = shot(&mut r, &mut view, &dry, &cam);
    let mut on = Vec::new();
    for _ in 0..3 {
        (on, _) = shot(&mut r, &mut view, &water, &cam);
    }
    assert!(on != empty, "the crests show over the rim");
    // The same plane drawn flat is hidden: the scene does test the cull.
    let flat = pool(Some(""));
    for _ in 0..2 {
        shot(&mut r, &mut view, &flat, &cam);
    }
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 1, "the rim alone");
    r.quality.gpu_occlusion_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &water, &cam);
    assert!(on == off, "Hi-Z hid the waved surface's crests");
}
