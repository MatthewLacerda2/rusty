//! The shooter-shaped worst case `make bench` measures (#835), loaded onto the
//! default fy_pool_day yard (#747) by `Harness.LoadStress`: skinned, animated
//! soldiers walking the navmesh with a Lua brain each; realtime point and spot
//! lights, some flickering like muzzle flashes and some casting shadows; a full
//! decal registry; a few particle systems. Pure placement — no RNG, no I/O — so
//! the scene is the same every run and two runs on one machine compare.

use glam::{Quat, Vec3};

use super::rig;
use crate::components::{AnimatorComponent, MaterialComponent, ScriptComponent};
use crate::core::curve::Range;
use crate::scene::authoring::{
    self, add_component, character_controller as cc, create_entity, nav_agent, particles,
    ComponentKind, Primitive,
};
use crate::scene::decal::DecalSpec;
use crate::scene::default_scene::layout::{ground_at, YARD_HALF};
use crate::scene::Scene;

/// What to spawn. [`Default`] is the issue's worst case.
#[derive(Clone, Debug, PartialEq)]
pub struct StressSpec {
    pub enemies: u32,
    /// Realtime lights, alternating point and spot.
    pub lights: u32,
    /// How many of those lights run `flicker_script` (muzzle flashes).
    pub flickering: u32,
    /// How many lights, from the first, cast shadows: half point lights (six
    /// atlas tiles each), half spots (one).
    pub shadowed: u32,
    /// Decals stamped; the registry keeps at most `MAX_DECALS`.
    pub decals: u32,
    pub particle_systems: u32,
    /// Each soldier's Lua brain, relative to the workspace.
    pub enemy_script: String,
    pub flicker_script: String,
}

impl Default for StressSpec {
    fn default() -> Self {
        Self {
            enemies: 50,
            lights: 32,
            flickering: 8,
            shadowed: 6,
            decals: 256,
            particle_systems: 4,
            enemy_script: String::new(),
            flicker_script: String::new(),
        }
    }
}

/// What was spawned, for the scenario to check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spawned {
    pub enemies: u32,
    pub lights: u32,
    pub decals: u32,
    pub particle_systems: u32,
}

/// Where the Player stands: 3 m in from its spawn, so the follow camera 4.5 m
/// behind it is inside the yard and looks down its length at the whole scene.
pub const PLAYER_AT: Vec3 = Vec3::new(0.0, 1.5, -15.0);

/// Spawn `spec` into `scene` (already holding the yard), and stand the Player at
/// [`PLAYER_AT`].
pub fn load(scene: &mut Scene, spec: &StressSpec) -> Spawned {
    if let Some(player) = scene.find_entity_by_name("Player") {
        scene.world.transform_mut(player).unwrap().position = PLAYER_AT;
    }
    let mut spawned = Spawned::default();
    for i in 0..spec.enemies {
        soldier(scene, i, &spec.enemy_script);
        spawned.enemies += 1;
    }
    for i in 0..spec.lights {
        let flicker = (i < spec.flickering).then_some(spec.flicker_script.as_str());
        light(scene, i, i < spec.shadowed, flicker);
        spawned.lights += 1;
    }
    for i in 0..spec.decals {
        let (point, normal) = decal_spot(i);
        let size = 0.3 + (i % 5) as f32 * 0.15;
        let color = if i % 3 == 0 {
            [0.35, 0.0, 0.0, 0.9]
        } else {
            [0.08, 0.07, 0.06, 0.9]
        };
        let spec = DecalSpec {
            size,
            rotation_deg: (i * 37 % 360) as f32,
            color,
            ..DecalSpec::default()
        };
        spawned.decals += u32::from(scene.spawn_decal(point, normal, spec).is_some());
    }
    for i in 0..spec.particle_systems {
        smoke(scene, i);
        spawned.particle_systems += 1;
    }
    scene.update_all_colliders();
    spawned
}

/// Soldier `i`: four columns on the deck either side of the pool, 2 m apart.
pub fn soldier_spot(i: u32) -> Vec3 {
    let x = [-12.5, -7.0, 7.0, 12.5][(i % 4) as usize];
    let z = -12.0 + (i / 4) as f32 * 2.0;
    Vec3::new(x, ground_at(x, z).unwrap_or(0.0), z)
}

fn soldier(scene: &mut Scene, i: u32, script: &str) {
    let id = scene.add_entity(format!("Soldier_{i}"));
    scene.world.transform_mut(id).unwrap().position = soldier_spot(i);
    scene.world.set_mesh(id, Some(rig::soldier()));
    let material = "enemy_red".to_string();
    scene
        .world
        .set_material(id, Some(MaterialComponent { material }));
    let mut anim = AnimatorComponent::default();
    anim.play(rig::WALK.to_string());
    scene.world.set_animator(id, Some(anim));
    add_component(scene, id, ComponentKind::CharacterController);
    if let Some(mut c) = scene.world.character_controller_mut(id) {
        cc::set_height(&mut c, 1.8);
        cc::set_radius(&mut c, 0.35);
        cc::set_center(&mut c, Vec3::Y * 0.9);
    }
    add_component(scene, id, ComponentKind::NavMeshAgent);
    if let Some(mut a) = scene.world.nav_agent_mut(id) {
        nav_agent::set_active(&mut a, true);
        nav_agent::set_radius(&mut a, 0.35);
        nav_agent::set_speed(&mut a, 3.0);
        nav_agent::set_base_offset(&mut a, 0.0);
        nav_agent::set_target(&mut a, soldier_spot(i));
    }
    // Hitboxes too: a shooter's enemies are shot at, so their bones carry colliders.
    let _ = scene.generate_hitboxes(id, &Default::default());
    attach(scene, id, script);
}

/// Light `i`: even ones are point lights along the side walls, odd ones spots
/// hanging over the pool and deck, pointing down.
fn light(scene: &mut Scene, i: u32, shadowed: bool, flicker: Option<&str>) {
    let row = (i / 2) as f32;
    let side = if (i / 2) % 2 == 0 { 1.0 } else { -1.0 };
    let z = -YARD_HALF.1 + 2.5 + row * 2.6;
    let (primitive, pos) = match i % 2 {
        0 => (Primitive::PointLight, Vec3::new(side * 13.5, 2.5, z)),
        _ => (Primitive::SpotLight, Vec3::new(side * 4.0, 4.0, z)),
    };
    let id = create_entity(scene, &format!("Light_{i}"), Some(primitive));
    let mut t = scene.world.transform_mut(id).unwrap();
    t.position = pos;
    t.rotation = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    drop(t);
    if let Some(mut l) = scene.world.light_mut(id) {
        authoring::light::set_color(&mut l, Vec3::new(1.0, 0.8, 0.55));
        authoring::light::set_intensity(&mut l, 2.0);
        authoring::light::set_range(&mut l, 9.0);
        authoring::light::set_cast_shadows(&mut l, shadowed);
    }
    if let Some(script) = flicker {
        attach(scene, id, script);
    }
}

/// Decal `i`: two in three on the deck around the pool, the rest on the side
/// walls, scattered by a fixed stride so they never stack.
fn decal_spot(i: u32) -> (Vec3, Vec3) {
    let along = -20.0 + (i * 13 % 400) as f32 * 0.1;
    if i % 3 == 2 {
        let x = if i % 2 == 0 { 14.75 } else { -14.75 };
        let y = 0.5 + (i * 7 % 25) as f32 * 0.1;
        return (Vec3::new(x, y, along), Vec3::new(-x.signum(), 0.0, 0.0));
    }
    let x = (if i % 2 == 0 { 1.0 } else { -1.0 }) * (5.5 + (i * 5 % 85) as f32 * 0.1);
    (Vec3::new(x, 0.0, along), Vec3::Y)
}

/// Smoke column `i`, in the yard's corners and by the jacuzzi.
fn smoke(scene: &mut Scene, i: u32) {
    let spots = [(-12.0, -16.0), (12.0, 16.0), (-12.0, 16.0), (10.0, 3.0)];
    let (x, z) = spots[i as usize % spots.len()];
    let id = create_entity(scene, &format!("Smoke_{i}"), None);
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.2, z);
    add_component(scene, id, ComponentKind::Particles);
    if let Some(mut p) = scene.world.particles_mut(id) {
        particles::set_rate(&mut p, 80.0);
        particles::set_max_particles(&mut p, 400);
        particles::set_lifetime(&mut p, Range { min: 2.0, max: 4.0 });
        particles::set_speed(&mut p, Range { min: 0.6, max: 1.4 });
        particles::set_spread(&mut p, 0.4);
    }
}

fn attach(scene: &mut Scene, id: u32, path: &str) {
    if path.is_empty() {
        return;
    }
    scene.world.scripts_mut(id).unwrap().push(ScriptComponent {
        path: path.to_string(),
        is_loaded: false,
        ..Default::default()
    });
}
