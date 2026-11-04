//! src/render/scene/inputs.rs — how the extract learns what changed (#1706).
//!
//! Every input a retained object holds is one of:
//! - **In the ECS**, read through a change cursor: the mesh (`MeshComponent`; its
//!   geometry, cast mode, small-object limits, probe use), the material slots
//!   (`MaterialComponent`), whether it is a light (`LightComponent`, which draws it
//!   unlit), the world matrix and its sign (`WorldPose`), activity (`Activity`), layer
//!   (`Layer`) and the static flag (`StaticSet`). The skinned palette is the one ECS
//!   input not stamped (`World::set_pose_palette`): read per camera.
//! - **On the `Scene`, outside the log**, by revision (`scene::revision`): one
//!   compare on a frame nothing wrote them. The probe field (`entity_probe_sh`) by
//!   [`crate::scene::ProbeVolume::revision`]; the material library
//!   (`material_entry_at`, `entity_params`) and the lightmap set (`solid_instance`)
//!   by their `Revisioned` revisions, kept with a copy compared only when a revision
//!   moved, so a write of the same value (a script setting a colour each frame)
//!   re-derives nothing. The shader overrides are compared whole (a handful of
//!   entries; an override diff names its entities).
//! - **On the renderer**, by value each frame: `lightmaps.resident()` and
//!   `.directional()`, `capture.static_only` and `capture.probe_bounce`; and by
//!   [`super::Retained::touch`] at their writes: a material group dropped
//!   (`invalidate_textures`, `gpu/textures/freshness.rs`), a surface shader gone
//!   stale (`SurfaceShaders::refresh`). The registered render textures have their own
//!   revision, `Retained::render_textures`, bumped by `sync_render_textures`: it
//!   re-derives only the objects whose materials name one, since the Scene and Game
//!   views register different sets and a `touch` there would rebuild on every view
//!   switch. A mesh
//!   (re-)upload marks only the objects drawing that geometry
//!   ([`super::MeshHandles::uploaded`]).
//! - **Per camera, on purpose** (`draw.rs`): `LodSelection`, the culling mask, the
//!   frustum, the small-object cull's eye, the view depth, `MotionHistory`, the
//!   reflection probe pick (`ProbeArray::probe_for`, whose resident probes follow the
//!   camera), the proxy volume (`proxy_volumes`), the palette, `self.instancing`, and a
//!   preview's forward pipeline (on the view).
//!
//! **Known gaps, for part 2** (the persistent material table):
//! - A material's shared param buffer (`ParamOwner::Material`) is keyed by library
//!   name across scenes, and `write_params` runs only when an object is re-derived,
//!   so scene A can draw with scene B's values for a same-named material whose
//!   surface shader has runtime params. No shipping path reaches it today.
//! - Any change to the lightmap set rebuilds every object, and a despawn removes the
//!   entity's lightmap entry: destroying one lightmapped prop re-derives the World.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{LightComponent, MaterialAsset, MaterialComponent, MeshComponent};
use crate::ecs::{Activity, Changes, Cursor, Layer, StaticSet, WorldPose};
use crate::scene::lighting::lightmap::LightmapSet;
use crate::scene::revision::{Revision, Revisioned};
use crate::scene::shader_overrides::ShaderOverrides;
use crate::scene::Scene;

/// The renderer-side inputs read by value: what a key or an instance was built under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RendererInputs {
    /// [`super::Retained`]'s revision.
    pub revision: u64,
    pub lightmaps_resident: bool,
    pub lightmaps_directional: bool,
    pub static_only: bool,
    pub probe_bounce: bool,
}

/// What changed since the last extract.
pub(super) enum Dirty {
    /// Re-derive every object.
    Rebuild,
    /// Re-derive `whole`; re-place `posed` (a move alone).
    Some {
        whole: BTreeSet<u32>,
        posed: BTreeSet<u32>,
    },
}

/// One scene's cursors and copies.
pub(super) struct Inputs {
    mesh: Cursor<MeshComponent>,
    material: Cursor<MaterialComponent>,
    light: Cursor<LightComponent>,
    pose: Cursor<WorldPose>,
    activity: Cursor<Activity>,
    layer: Cursor<Layer>,
    statics: Cursor<StaticSet>,
    library: Kept<BTreeMap<String, MaterialAsset>>,
    overrides: ShaderOverrides,
    lightmaps: Kept<LightmapSet>,
    probes: Revision,
    renderer: RendererInputs,
    /// The mesh upload count it has seen (`MeshHandles::uploads`).
    pub uploads_seen: u64,
    /// The render-texture revision it has seen (`Retained::render_textures`).
    pub render_textures_seen: u64,
}

impl Inputs {
    /// Open the cursors on `scene` and copy what it holds now.
    pub fn open(scene: &Scene, renderer: RendererInputs) -> Self {
        let w = &scene.world;
        Self {
            mesh: w.change_cursor(),
            material: w.change_cursor(),
            light: w.change_cursor(),
            pose: w.change_cursor(),
            activity: w.change_cursor(),
            layer: w.change_cursor(),
            statics: w.change_cursor(),
            library: Kept::of(&scene.materials),
            overrides: scene.shader_overrides.clone(),
            lightmaps: Kept::of(&scene.lightmaps),
            probes: scene.probes.revision(),
            renderer,
            uploads_seen: 0,
            render_textures_seen: 0,
        }
    }

    /// What changed in `scene` since the last read, every cursor and copy brought up
    /// to date.
    pub fn read(&mut self, scene: &Scene, renderer: RendererInputs) -> Dirty {
        let w = &scene.world;
        let mut whole = BTreeSet::new();
        let mut rebuild = false;
        let mut take = |changes: Changes, into: &mut BTreeSet<u32>| match changes {
            Changes::Rebuild => rebuild = true,
            Changes::Ids(ids) => into.extend(ids),
        };
        take(w.changes(&mut self.mesh), &mut whole);
        take(w.changes(&mut self.material), &mut whole);
        take(w.changes(&mut self.light), &mut whole);
        take(w.changes(&mut self.activity), &mut whole);
        take(w.changes(&mut self.layer), &mut whole);
        take(w.changes(&mut self.statics), &mut whole);
        let mut posed = BTreeSet::new();
        take(w.changes(&mut self.pose), &mut posed);
        rebuild |= std::mem::replace(&mut self.renderer, renderer) != renderer;
        rebuild |= self.library.refresh(&scene.materials);
        rebuild |= self.lightmaps.refresh(&scene.lightmaps);
        let probes = scene.probes.revision();
        rebuild |= std::mem::replace(&mut self.probes, probes) != probes;
        if self.overrides != scene.shader_overrides {
            whole.extend(override_diff(&self.overrides, &scene.shader_overrides));
            self.overrides.clone_from(&scene.shader_overrides);
        }
        if rebuild {
            return Dirty::Rebuild;
        }
        posed.retain(|id| !whole.contains(id));
        Dirty::Some { whole, posed }
    }
}

/// A copy of a `Revisioned` scene value and the revision it was taken at.
struct Kept<T> {
    copy: T,
    seen: Revision,
}

impl<T: Clone + PartialEq> Kept<T> {
    fn of(now: &Revisioned<T>) -> Self {
        Self {
            copy: (**now).clone(),
            seen: now.revision(),
        }
    }

    /// Bring the copy up to `now`; whether its value changed. One compare of the
    /// revisions when nothing wrote it; the value compared only when one did.
    fn refresh(&mut self, now: &Revisioned<T>) -> bool {
        if std::mem::replace(&mut self.seen, now.revision()) == now.revision() {
            return false;
        }
        if self.copy == **now {
            return false;
        }
        self.copy.clone_from(now);
        true
    }
}

/// The entities whose overrides differ between `old` and `new`.
fn override_diff(old: &ShaderOverrides, new: &ShaderOverrides) -> BTreeSet<u32> {
    let ids = old.entities().chain(new.entities());
    ids.filter(|&id| old.of(id) != new.of(id)).collect()
}
