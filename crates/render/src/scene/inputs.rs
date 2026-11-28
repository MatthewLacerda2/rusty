//! Every input a retained object holds, by how it is held:
//! - the ECS, by change cursor: mesh, material slots, light, `WorldPose`, `Activity`, `Layer`,
//!   `StaticSet`;
//! - the `Scene`, by revision: each probe set (re-sampled in reach), material library,
//!   lightmap set (shader overrides compared whole);
//! - the renderer, by value each frame (`RendererInputs`) or `Retained::touch` at the write.
//!   Render textures keep their own revision, since a `touch` would rebuild on every
//!   Scene/Game view switch;
//! - between ticks, the drawn pose (`DrawPoses::repose`);
//! - per camera, on purpose (`draw.rs`): LOD, mask, frustum, motion history, probe pick, the
//!   skinned palette (`World::set_pose_palette` stamps no change).

use std::collections::{BTreeMap, BTreeSet};

use super::records::{library_diff, params_moved, rekeys};
use rusty_components::{
    LightComponent, LodGroupComponent, MaterialAsset, MaterialComponent, MeshComponent,
};
use rusty_ecs::{Activity, Changes, Cursor, Layer, StaticSet, WorldPose};
use rusty_scene::lighting::lightmap_set::LightmapSet;
use rusty_scene::lighting::probe::change::{FieldChange, SetStamp};
use rusty_scene::revision::{Revision, Revisioned};
use rusty_scene::shader_overrides::ShaderOverrides;
use rusty_scene::Scene;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RendererInputs {
    pub revision: u64,
    pub lightmaps_resident: bool,
    pub lightmaps_directional: bool,
    pub static_only: bool,
    pub probe_bounce: bool,
}

pub(super) enum Dirty {
    Rebuild,
    Some {
        whole: BTreeSet<u32>,
        posed: BTreeSet<u32>,
    },
}

#[derive(Default)]
pub(super) struct LibraryEdits {
    pub rekeyed: BTreeSet<String>,
    pub rewritten: BTreeMap<String, bool>,
}

pub(super) struct Inputs {
    mesh: Cursor<MeshComponent>,
    material: Cursor<MaterialComponent>,
    light: Cursor<LightComponent>,
    pose: Cursor<WorldPose>,
    activity: Cursor<Activity>,
    layer: Cursor<Layer>,
    statics: Cursor<StaticSet>,
    lod_groups: Cursor<LodGroupComponent>,
    library: Kept<Library>,
    overrides: ShaderOverrides,
    lightmaps: Kept<LightmapSet>,
    probes: Vec<SetStamp<Revision>>,
    pub probe_change: Option<FieldChange>,
    renderer: RendererInputs,
    pub uploads_seen: u64,
    pub render_textures_seen: u64,
    pub regroup: bool,
}

impl Inputs {
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
            probes: probe_stamps(scene),
            probe_change: None,
            renderer,
            uploads_seen: 0,
            render_textures_seen: 0,
            regroup: true,
        }
    }

    pub fn read(&mut self, scene: &Scene, renderer: RendererInputs) -> (Dirty, LibraryEdits) {
        let w = &scene.world;
        let mut whole = BTreeSet::new();
        let mut rebuild = false;
        let mut take = |changes: Changes, into: &mut BTreeSet<u32>| match changes {
            Changes::Rebuild => rebuild = true,
            Changes::Ids(ids) => into.extend(ids),
        };
        let meshes = w.changes(&mut self.mesh);
        let groups = w.changes(&mut self.lod_groups);
        self.regroup = !(meshes.is_empty() && groups.is_empty());
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
        let library = self.library.edited(&scene.materials);
        rebuild |= self.lightmaps.refresh(&scene.lightmaps);
        let seen = self.probes.iter().map(|s| (s.handle, s.key));
        if !seen.eq(scene.with_probe_field(|f| f.revisions())) {
            let now = probe_stamps(scene);
            self.probe_change = FieldChange::between(&self.probes, &now);
            self.probes = now;
        }
        if self.overrides != scene.shader_overrides {
            whole.extend(override_diff(&self.overrides, &scene.shader_overrides));
            self.overrides.clone_from(&scene.shader_overrides);
        }
        if rebuild {
            self.regroup = true;
            return (Dirty::Rebuild, library);
        }
        posed.retain(|id| !whole.contains(id));
        (Dirty::Some { whole, posed }, library)
    }
}

fn probe_stamps(scene: &Scene) -> Vec<SetStamp<Revision>> {
    let sets = scene.probe_sets().into_iter();
    sets.map(|(h, v)| SetStamp::of(h, v.revision(), v))
        .collect()
}

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

pub(super) type Library = BTreeMap<String, MaterialAsset>;

impl Kept<Library> {
    fn edited(&mut self, now: &Revisioned<Library>) -> LibraryEdits {
        let mut edits = LibraryEdits::default();
        if std::mem::replace(&mut self.seen, now.revision()) == now.revision() {
            return edits;
        }
        for name in library_diff(&self.copy, now) {
            match (self.copy.get(&name), now.get(&name)) {
                (Some(old), Some(new)) if !rekeys(old, new) => {
                    edits.rewritten.insert(name.clone(), params_moved(old, new));
                }
                _ => drop(edits.rekeyed.insert(name.clone())),
            }
            match now.get(&name) {
                Some(asset) => self.copy.insert(name, asset.clone()),
                None => self.copy.remove(&name),
            };
        }
        edits
    }
}

fn override_diff(old: &ShaderOverrides, new: &ShaderOverrides) -> BTreeSet<u32> {
    let ids = old.entities().chain(new.entities());
    ids.filter(|&id| old.of(id) != new.of(id)).collect()
}
