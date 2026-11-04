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
//!   re-derives nothing. The instance's `in_bake` (#1750: SSIL gathers no emission a
//!   bake took in) reads the static flag, activity, the light and mesh cursors and
//!   the lightmap set, its baked-light record included: a re-bake that records
//!   other lights changes the set's content, so it is seen. The shader overrides are compared whole (a handful of
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
//! **What the shadow sweeps read** (`passes::shadows`, part 2 of #1706), off the
//! objects but for what the frame decides:
//! - **Kept, through the cursors above:** cast mode (`MeshComponent`), activity,
//!   the static flag, the small-object limits, the world matrix and its sign (a
//!   mirrored caster's facing), the resident runs and their slots' Render Faces,
//!   each slot's clip (`SlotDraw::clip`: its surface pipeline's cut, its Cutout
//!   test, its material group and record, so `forget_groups`' `Retained::touch`
//!   re-derives it with the camera's key), whether a moving variant cuts any slot
//!   (`RenderObject::moving`), and the static bake's digest terms
//!   (`ObjectCasting`: cast mode, small-object cull, faces, Cutout tests).
//! - **Kept, by a cursor of its own:** which renderers a `LODGroup` keeps out of
//!   the bakes (`RenderScene::unbaked`), re-read when a `LodGroupComponent` or a
//!   mesh changed. The digest (`RenderScene::statics_key`) is refolded from the
//!   objects only when a static one was re-derived or `unbaked` moved.
//! - **Per frame, on purpose:** the base camera's LOD level and small-object
//!   verdict, and a palette object's palette and posed bounds (unstamped).
//!
//! **The baked PVS** (#1714, `occlusion.rs`, outside the objects): bound whole per
//! scene when `Scene::occlusion`'s revision moved, on a rebuild, or when the
//! `LODGroup`s' renderers moved (read when `regroup` is set); a static object
//! re-derived or moved re-checks that object alone. A camera reads its cell's row
//! per frame, on purpose.
//!
//! **Decals** (`decals::kept`, outside the objects): each decal's maps are kept by
//! decal id, re-read when it names another material or texture or the scene or
//! library revision moved; its owner's pose and its age's fade are read every
//! frame, so its record is rebuilt per frame and uploaded only when the records
//! differ from what the buffer holds.
//!
//! **Known gaps, for part 3** (the persistent material table):
//! - A material's shared param buffer (`ParamOwner::Material`) is keyed by library
//!   name across scenes, and `write_params` runs only when an object is re-derived,
//!   so scene A can draw with scene B's values for a same-named material whose
//!   surface shader has runtime params. No shipping path reaches it today. Part 2
//!   widened it: the per-frame shadow clip walk (`shadow_clips`) called
//!   `write_params` for every clipped slot, which hid the gap for cutting
//!   materials. With a preview tab beside the main scene, a same-named cutting
//!   material with other runtime params now has the main scene's shadow cut and
//!   camera draw read the preview's values until the main scene's object is
//!   re-derived.
//! - Any change to the lightmap set rebuilds every object, and a despawn removes the
//!   entity's lightmap entry: destroying one lightmapped prop re-derives the World.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{
    LightComponent, LodGroupComponent, MaterialAsset, MaterialComponent, MeshComponent,
};
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
    lod_groups: Cursor<LodGroupComponent>,
    library: Kept<BTreeMap<String, MaterialAsset>>,
    overrides: ShaderOverrides,
    lightmaps: Kept<LightmapSet>,
    probes: Revision,
    renderer: RendererInputs,
    /// The mesh upload count it has seen (`MeshHandles::uploads`).
    pub uploads_seen: u64,
    /// The render-texture revision it has seen (`Retained::render_textures`).
    pub render_textures_seen: u64,
    /// A `LODGroup` or a mesh changed in the last read: which renderers the groups
    /// keep out of the static bakes is re-read (`UnbakedCasters`).
    pub regroup: bool,
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
            lod_groups: w.change_cursor(),
            library: Kept::of(&scene.materials),
            overrides: scene.shader_overrides.clone(),
            lightmaps: Kept::of(&scene.lightmaps),
            probes: scene.probes.revision(),
            renderer,
            uploads_seen: 0,
            render_textures_seen: 0,
            regroup: true,
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
        let meshes = w.changes(&mut self.mesh);
        let groups = w.changes(&mut self.lod_groups);
        let none = |c: &Changes| matches!(c, Changes::Ids(ids) if ids.is_empty());
        self.regroup = !(none(&meshes) && none(&groups));
        take(meshes, &mut whole);
        take(groups, &mut BTreeSet::new());
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
            self.regroup = true;
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
