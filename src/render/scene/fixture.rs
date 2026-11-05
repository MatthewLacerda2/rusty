//! src/render/scene/fixture.rs — what the retained scene's tests share: a scene of
//! primitives, the objects a renderer holds for it, and the objects a build from
//! scratch gives.

use glam::Vec3;

use super::{RenderObject, RenderScene};
use crate::components::{MaterialAsset, MaterialComponent, RenderMode};
use crate::render::gpu::uniforms::ObjectData;
use crate::render::{MaterialData, RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

pub(super) const RES: u32 = 32;

/// `n` primitives on a row (boxes, spheres, planes), every third red, every fifth
/// glass, a sun above; the library holds `red` and `glass`.
pub(super) fn yard(n: u32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let red = MaterialAsset {
        base_color: [0.9, 0.1, 0.1],
        ..MaterialAsset::default()
    };
    let glass = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.4,
        ..MaterialAsset::default()
    };
    scene.materials.insert("red".into(), red);
    scene.materials.insert("glass".into(), glass);
    for i in 0..n {
        let kind = [Primitive::Box, Primitive::Sphere, Primitive::Plane][i as usize % 3];
        let id = create_entity(&mut scene, &format!("Prop_{i}"), Some(kind));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(i as f32 - 2.0, 0.0, -4.0);
        let material = match i {
            i if i % 5 == 4 => Some("glass"),
            i if i % 3 == 1 => Some("red"),
            _ => None,
        };
        let material = material.map(|m| MaterialComponent::new(m.to_string()));
        scene.world.set_material(id, material);
    }
    scene
}

/// The camera every test looks through: down the row.
pub(super) fn eye() -> Camera {
    Camera::new(Vec3::new(0.0, 1.0, 3.0), -90.0, -10.0)
}

/// Render `scene` once into a fresh view; its pixels and counters.
pub(super) fn frame(r: &mut Renderer, scene: &Scene) -> (Vec<u8>, RenderCounters) {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, RES, RES);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, scene, &eye(), &out, false);
    let texture = view.color_target().unwrap();
    let px = crate::render::readback::read_texture_rgba8(&r.device, &r.queue, texture, RES, RES);
    (px, r.frame_counters)
}

/// One object as a test compares it: its id, the object with its slot number
/// blanked (a kept and a fresh build allocate different slots), what its slot of
/// the object table holds, and what each material slot's record holds.
pub(super) type Held = (u32, RenderObject, ObjectData, Vec<MaterialData>);

fn as_held(r: &Renderer, rs: &RenderScene) -> Vec<Held> {
    let table = r.retained.records.table();
    let held = rs.objects().map(|(id, o)| {
        let records = o.slots.iter().map(|s| *table.get(s.key.record));
        let object = RenderObject {
            slot: 0,
            ..o.clone()
        };
        (
            id,
            object,
            *r.retained.objects.get(o.slot),
            records.collect(),
        )
    });
    held.collect()
}

/// The objects `r` holds for `scene` now, in draw order, with their records. Every
/// kept object has a slot of its own.
pub(super) fn held(r: &Renderer, scene: &Scene) -> Vec<Held> {
    let kept = r.retained.scene(scene.id()).expect("extracted");
    let mut slots: Vec<u32> = kept.objects().map(|(_, o)| o.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(
        slots.len(),
        kept.objects().count(),
        "two objects share a slot"
    );
    assert!(!slots.contains(&0), "slot 0 is the identity's");
    as_held(r, kept)
}

/// The objects a build of `scene` from scratch gives, through `r`'s own handles
/// and resident buffers, with their records; their slots are given back after.
pub(super) fn fresh(r: &mut Renderer, scene: &Scene) -> Vec<Held> {
    let mut rs = RenderScene::default();
    r.rebuild_objects(scene, &mut rs);
    let held = as_held(r, &rs);
    rs.release(&mut r.retained.objects);
    held
}

/// The tables are whole: every object slot in use is a kept object's (or the
/// identity's), every material slot in use a kept record's (or the flat one's), and
/// the GPU holds what each table holds, slot for slot.
pub(super) fn table_holds(r: &mut Renderer) -> Result<(), String> {
    let live = (r.retained.objects.live(), r.retained.kept_objects() + 1);
    let kept = &r.retained.records;
    let records = (kept.table().live(), kept.kept().len() + 1);
    if live.0 != live.1 || records.0 != records.1 {
        return Err(format!(
            "live / kept: {live:?} objects, {records:?} records"
        ));
    }
    r.retained.records.classes_hold()?;
    let gpu = r.objects_on_gpu() == r.retained.objects.records().0;
    match gpu && r.records_on_gpu() == r.retained.records.table().records().0 {
        true => Ok(()),
        false => Err("the GPU's records differ from a table's".into()),
    }
}

/// Run `f` on entity `id` of `scene`, when there is one.
pub(super) fn on(scene: &mut Scene, id: Option<u32>, f: impl FnOnce(&mut Scene, u32)) {
    if let Some(id) = id {
        f(scene, id);
    }
}

/// Add a probe at `x` along +X, or for an odd `x` write the first probe's
/// shadowmask occlusion (#1695, `ObjectData::occlusion`).
pub(super) fn probe(scene: &mut Scene, x: u8) {
    match (x % 2, scene.probes.probes_mut().first_mut()) {
        (1, Some(p)) => p.occlusion = glam::Vec4::splat(0.25 * f32::from(x)),
        _ => drop(scene.probes.add_probe(Vec3::X * f32::from(x))),
    }
}
