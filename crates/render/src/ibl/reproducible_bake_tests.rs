//! A bake is a function of the scene's static content alone: the clock, the cubes an
//! earlier bake left and the order the probes bake in never reach its bytes.

use std::path::Path;

use glam::{Quat, Vec3};
use rusty_components::particle::{Particle, ParticleEmitterComponent};
use rusty_components::{MaterialAsset, MaterialComponent};
use rusty_scene::{MeshComponent, Scene};
use rusty_shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use rusty_shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

use super::cube_file;
use super::reflection_bake_tests::colored_room;

/// A surface shader whose emissive breathes with game time (`pulse_glow`).
struct Pulse(String);

impl Pulse {
    fn new() -> Self {
        let name = format!("test_pulse_2423_{}", std::process::id());
        let recipe = ShaderRecipe {
            pass: PassKind::Surface,
            name: name.clone(),
            blocks: vec![BlockSel {
                id: "pulse_glow".into(),
                params: Default::default(),
            }],
        };
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).expect("bake succeeds");
        Self(name)
    }
}

impl Drop for Pulse {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// The coloured room, a mirror floor each probe sees, a pulsing wall, and two probes.
fn pool(pulse: &Pulse) -> Scene {
    let mut scene = colored_room();
    let mirror = MaterialAsset {
        base_color: [1.0; 3],
        metallic: 1.0,
        roughness: 0.05,
        ..MaterialAsset::default()
    };
    scene.materials.insert("mirror".into(), mirror);
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    let id = scene.add_entity("Mirror".into());
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene.world.set_static(id, true);
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (Vec3::NEG_Y * 2.0, Vec3::new(4.0, 0.2, 4.0));
        t.rotation = Quat::IDENTITY;
    }
    let material = MaterialComponent::new("mirror".to_string());
    scene.world.set_material(id, Some(material));
    let wall = scene.materials.get_mut("PosX_mat").unwrap();
    wall.shader = Some(pulse.0.clone());
    for x in [-1.0, 1.0] {
        let i = (scene.reflection_probes).add(Vec3::X * x, Vec3::splat(5.0));
        scene.reflection_probes.edit(i, |p| p.manual = true);
    }
    scene.lighting_settings.reflections.resolution = 32;
    scene
}

fn cubes(dir: &Path) -> Vec<Vec<u8>> {
    (0..2)
        .map(|i| std::fs::read(cube_file(dir, i)).unwrap())
        .collect()
}

#[test]
fn gpu_a_rebake_with_the_clock_moved_and_its_cubes_loaded_writes_the_same_bytes() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(64, 64) else {
        return;
    };
    let pulse = Pulse::new();
    let dir = rusty_test_support::temp::dir().join("rusty_2423_rebake");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut first = pool(&pulse);
    renderer.bake_reflections(&mut first, &dir).unwrap();
    let baked = cubes(&dir);

    // The scene as saved after that bake: its probes name the cubes it wrote.
    let mut again = pool(&pulse);
    again.shader_time = 0.37; // mid-pulse
    for (i, probe) in first.reflection_probes.probes.iter().enumerate() {
        (again.reflection_probes).set_cubemap(i, probe.cubemap_path.clone());
    }
    renderer.bake_reflections(&mut again, &dir).unwrap();
    assert!(baked == cubes(&dir), "the second bake wrote other bytes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gpu_a_live_particle_never_bakes_into_a_probe() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(64, 64) else {
        return;
    };
    let probe = rusty_scene::ReflectionProbe::new(Vec3::ZERO, Vec3::splat(5.0));
    let bare = renderer.bake_reflection(&colored_room(), &probe, 32).bytes;
    let mut smoky = colored_room();
    let id = smoky.add_entity("Smoke".to_string());
    let mut emitter = ParticleEmitterComponent::default();
    let puff = Particle::at(Vec3::new(0.0, 0.0, -2.0), 2.0, [1.0; 4], 99.0);
    emitter.runtime.particles = vec![puff];
    smoky.world.set_particles(id, Some(emitter));
    let smoked = renderer.bake_reflection(&smoky, &probe, 32).bytes;
    assert!(bare == smoked, "the puff reached the cube");
}
