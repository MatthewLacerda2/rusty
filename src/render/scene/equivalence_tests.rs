//! src/render/scene/equivalence_tests.rs — the retained objects after any edit
//! sequence equal a build from scratch (#1706), as `ecs/changes/derived_tests.rs`
//! proves the change log's consumers: the ECS write paths of `ops_tests`, the inputs
//! the log does not cover (the library, overrides, lightmaps and a bake's light
//! record (#1750), probes, a capture, a re-uploaded shared mesh), the shadow sweeps'
//! key (`shadow_ops`) and the baked PVS's binding (`occlusion_ops`). One renderer.

use glam::Vec3;
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

use super::fixture::{fresh, held, RES};
use super::occlusion_ops::{self, baked_yard};
use super::shadow_ops::{self, s_op, S};
use crate::components::mesh::MeshRevision;
use crate::components::{LightComponent, MaterialComponent};
use crate::ecs::changes::ops_tests::{apply, op, Op};
use crate::render::Renderer;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::Scene;

#[derive(Clone, Debug)]
enum R {
    World(Op),
    Prop(u8),
    Material(usize, Option<bool>),
    Library(u8),
    Override(usize, bool),
    Light(usize, bool),
    Lightmap(usize, u8),
    Probe(u8),
    Capture(bool),
    Unmesh(usize),
    Reshape(usize),
    Rewind,
    Shadow(S),
    Frame,
}

fn r_op() -> impl Strategy<Value = R> {
    let at = || 0usize..12;
    prop_oneof![
        8 => op().prop_map(R::World),
        2 => (0u8..3).prop_map(R::Prop),
        2 => (at(), proptest::option::of(any::<bool>())).prop_map(|(i, m)| R::Material(i, m)),
        1 => (0u8..4).prop_map(R::Library),
        2 => (at(), any::<bool>()).prop_map(|(i, s)| R::Override(i, s)),
        1 => (at(), any::<bool>()).prop_map(|(i, l)| R::Light(i, l)),
        1 => (at(), 0u8..4).prop_map(|(i, l)| R::Lightmap(i, l)),
        1 => (0u8..4).prop_map(R::Probe),
        1 => any::<bool>().prop_map(R::Capture),
        1 => at().prop_map(R::Unmesh),
        1 => at().prop_map(R::Reshape),
        1 => Just(R::Rewind),
        3 => s_op().prop_map(R::Shadow),
        4 => Just(R::Frame),
    ]
}

/// Apply `op` to `scene` (and, for a capture, to `r`).
fn edit(r: &mut Renderer, scene: &mut Scene, op: &R) {
    let pick = |s: &Scene, i: usize| s.world.ids().get(i).copied();
    let kinds = [Primitive::Box, Primitive::Sphere, Primitive::Plane];
    match *op {
        R::World(ref op) => apply(&mut scene.world, op),
        R::Prop(k) => {
            create_entity(scene, "prop", Some(kinds[k as usize]));
        }
        R::Library(c) => scene.materials.get_mut("red").unwrap().base_color[0] = f32::from(c),
        R::Probe(x) => {
            scene.probes.add_probe(Vec3::X * f32::from(x));
        }
        R::Capture(on) => r.capture.static_only = on,
        R::Rewind => scene.world.rewind_changes(),
        R::Shadow(ref op) => shadow_ops::edit(scene, op),
        R::Frame => {}
        R::Material(i, m) => on(scene, pick(scene, i), |s, id| {
            let name = m.map(|red| if red { "red" } else { "glass" }.to_string());
            s.world.set_material(id, name.map(MaterialComponent::new));
        }),
        R::Override(i, set) => on(scene, pick(scene, i), |s, id| match set {
            true => s.shader_overrides.set(id, "tint".into(), vec![1.0]),
            false => s.shader_overrides.clear(id, None),
        }),
        R::Light(i, l) => on(scene, pick(scene, i), |s, id| {
            let first = s.world.ids_with_light().first().copied();
            let sun: Option<LightComponent> =
                first.and_then(|f| s.world.light(f).map(|c| (*c).clone()));
            s.world.set_light(id, sun.filter(|_| l));
        }),
        R::Lightmap(i, l) => on(scene, pick(scene, i), |s, id| match l {
            0 => drop(s.lightmaps.remove(id)),
            1 => drop(s.lightmaps.lights.insert(id)),
            2 => drop(s.lightmaps.lights.remove(&id)),
            _ => s.lightmaps.insert(LightmapEntry {
                entity: id,
                page: 0,
                scale_offset: [0.5, 0.5, 0.0, 0.0],
            }),
        }),
        R::Unmesh(i) => on(scene, pick(scene, i), |s, id| {
            drop(s.world.set_mesh(id, None))
        }),
        R::Reshape(i) => on(scene, pick(scene, i), |s, id| {
            if let Some(mut mesh) = s.world.mesh_mut(id) {
                mesh.vertices.iter_mut().for_each(|v| v.position[1] *= 1.5);
                mesh.revision = MeshRevision::next(); // every sharer's buffers change
            }
        }),
    }
}

fn on(scene: &mut Scene, id: Option<u32>, f: impl FnOnce(&mut Scene, u32)) {
    let Some(id) = id else { return };
    f(scene, id);
}

/// After every frame of `ops`, the objects `r` holds equal a fresh build.
fn holds(r: &mut Renderer, ops: &[R]) -> Result<(), TestCaseError> {
    let mut scene = baked_yard(6);
    r.capture.static_only = false;
    r.extract_scene(&scene);
    for op in ops.iter().chain([&R::Frame]) {
        edit(r, &mut scene, op);
        if matches!(op, R::Frame) {
            r.extract_scene(&scene);
            prop_assert_eq!(held(r, &scene), fresh(r, &scene), "after {:?}", ops);
            let shadows = shadow_ops::kept_equals_fresh(r, &scene)
                .and_then(|()| occlusion_ops::kept_equals_fresh(r, &scene));
            prop_assert!(shadows.is_ok(), "{:?} after {:?}", shadows, ops);
        }
    }
    Ok(())
}

#[test]
fn gpu_any_edit_sequence_extracts_what_a_fresh_build_holds() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut runner = TestRunner::new(Config {
        cases: 48,
        ..Config::default()
    });
    let ops = proptest::collection::vec(r_op(), 1..40);
    let r = std::cell::RefCell::new(&mut r);
    let result = runner.run(&ops, |ops| holds(&mut r.borrow_mut(), &ops));
    result.unwrap_or_else(|e| panic!("{e}"));
}
