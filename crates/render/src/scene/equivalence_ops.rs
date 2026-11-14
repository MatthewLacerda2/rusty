//! crates/render/src/scene/equivalence_ops.rs — the edits `equivalence_tests` draws its
//! sequences from (#1706): the ECS write paths of `changes::ops`, the inputs the log
//! does not cover, and the shadow ops.

use proptest::prelude::*;

use super::fixture::{on, probe};
use super::shadow_ops::{self, s_op, S};
use crate::Renderer;
use rusty_components::mesh::MeshRevision;
use rusty_components::{LightComponent, MaterialComponent, RenderMode};
use rusty_ecs::changes::ops::{apply, Op};
use rusty_scene::authoring::{create_entity, Primitive};
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::Scene;

#[derive(Clone, Debug)]
pub(super) enum R {
    World(Op),
    Prop(u8),
    /// Entity, material, and whether two slots name it (`set_slot` pads a list).
    Material(usize, Option<u8>, bool),
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
    /// An edit of the second scene's library, which holds the first's names.
    Other(u8),
    /// A fixed tick recorded for drawing between ticks (#1877), a blend set (in
    /// quarters of a tick), or the blend dropped.
    Tick,
    Blend(u8),
    Unblend,
    Frame,
}

pub(super) fn r_op() -> impl Strategy<Value = R> {
    let at = || 0usize..12;
    prop_oneof![
        8 => rusty_ecs::edit_ops!().prop_map(R::World),
        2 => (0u8..3).prop_map(R::Prop),
        3 => (at(), proptest::option::of(0u8..3), any::<bool>())
            .prop_map(|(i, m, two)| R::Material(i, m, two)),
        3 => (0u8..10).prop_map(R::Library),
        1 => (0u8..9).prop_map(R::Other),
        2 => (at(), any::<bool>()).prop_map(|(i, s)| R::Override(i, s)),
        1 => (at(), any::<bool>()).prop_map(|(i, l)| R::Light(i, l)),
        1 => (at(), 0u8..4).prop_map(|(i, l)| R::Lightmap(i, l)),
        1 => (0u8..4).prop_map(R::Probe),
        1 => any::<bool>().prop_map(R::Capture),
        1 => at().prop_map(R::Unmesh),
        1 => at().prop_map(R::Reshape),
        1 => Just(R::Rewind),
        3 => s_op().prop_map(R::Shadow),
        2 => Just(R::Tick),
        2 => (0u8..5).prop_map(R::Blend),
        1 => Just(R::Unblend),
        4 => Just(R::Frame),
    ]
}

/// Apply `op` to `scene` (and, for a capture, to `r`).
pub(super) fn edit(r: &mut Renderer, scene: &mut Scene, op: &R) {
    let pick = |s: &Scene, i: usize| s.world.ids().get(i).copied();
    let kinds = [Primitive::Box, Primitive::Sphere, Primitive::Plane];
    match *op {
        R::World(ref op) => apply(&mut scene.world, op),
        R::Prop(k) => {
            create_entity(scene, "prop", Some(kinds[k as usize]));
        }
        R::Library(c) => library(scene, c),
        R::Probe(x) => probe(scene, x),
        R::Capture(on) => r.capture.static_only = on,
        R::Rewind => scene.world.rewind_changes(),
        R::Shadow(ref op) => shadow_ops::edit(scene, op),
        R::Tick | R::Blend(_) | R::Unblend => between_ticks(r, scene, op),
        R::Frame | R::Other(_) => {}
        R::Material(i, m, two) => on(scene, pick(scene, i), |s, id| {
            let name = m.map(|m| ["red", "glass", "blue"][m as usize].to_string());
            let slots = name.map(|n| vec![n; 1 + usize::from(two)]);
            let slots = slots.map(|materials| MaterialComponent { materials });
            s.world.set_material(id, slots);
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
            s.world.set_mesh(id, None);
        }),
        R::Reshape(i) => on(scene, pick(scene, i), |s, id| {
            if let Some(mut mesh) = s.world.mesh_mut(id) {
                mesh.vertices.iter_mut().for_each(|v| v.position[1] *= 1.5);
                mesh.revision = MeshRevision::next(); // every sharer's buffers change
            }
        }),
    }
}

/// A tick recorded, a blend set or dropped (#1877).
fn between_ticks(r: &mut Renderer, scene: &Scene, op: &R) {
    match *op {
        R::Tick => r.draw_poses.record(scene),
        R::Blend(q) => r.draw_poses.set_alpha(f32::from(q) / 4.0),
        _ => r.draw_poses.clear(),
    }
}

/// Edit the library: `red`'s colour or metallic, `glass`'s alpha (a record's
/// alone) or sorting priority (a slot's), `red` turned Cutout or back or its cutoff (a key's), or add or remove
/// `blue`, which objects may name while it is missing.
pub(super) fn library(scene: &mut Scene, c: u8) {
    let m = &mut scene.materials;
    let Some(red) = m.get_mut("red").filter(|_| c <= 6 && c != 3) else {
        match c {
            3 => m.get_mut("glass").unwrap().alpha = 0.2,
            9 => m.get_mut("glass").unwrap().sorting_priority ^= 1, // #2039
            _ if m.remove("blue").is_some() => {}
            _ => drop(m.insert("blue".into(), Default::default())),
        }
        return;
    };
    match c {
        4 => red.metallic = 1.0 - red.metallic,
        5 if red.render_mode == RenderMode::Cutout => red.render_mode = RenderMode::Opaque,
        5 => red.render_mode = RenderMode::Cutout,
        6 => red.alpha_cutoff = 1.0 - red.alpha_cutoff * 0.5,
        _ => red.base_color[0] = f32::from(c),
    }
}
