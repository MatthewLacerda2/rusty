//! One mark per field of the inventory (#1649): a way to set a distinctive value
//! on a scene and to see whether it is still there, which the lifecycle and keyed
//! tests run through every lifecycle. A field with no mark fails
//! `every_field_has_a_mark`, so a new field can't land without one.
//!
//! A field made of two classes (a probe set: placement and its baked payload) has
//! a mark per part, the second named `field.part` and held to its own class.

use std::sync::OnceLock;

use glam::Vec3;

use super::Class;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::decal::DecalSpec;
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::lighting::reflection_probe::ReflectionProbe;
use crate::scene::{Scene, SceneId};

/// One field's mark (or one part's).
pub struct Mark {
    /// `field`, or `field.part`.
    pub name: &'static str,
    /// The class a part is held to, when not its field's.
    pub part: Option<Class>,
    /// Set the mark on `scene`; `base` is the base entity.
    pub set: fn(&mut Scene, u32),
    /// Whether `scene` holds the mark.
    pub is_set: fn(&Scene, u32) -> bool,
}

impl Mark {
    /// The field the mark is on.
    pub fn field(&self) -> &'static str {
        self.name.split('.').next().unwrap_or(self.name)
    }
}

/// The scene every mark is set on: one box (the base entity), one light probe and
/// one reflection probe, so a mark can change a probe without changing the count.
pub fn base() -> (Scene, u32) {
    let mut scene = Scene::new();
    let base = create_entity(&mut scene, "Base", Some(Primitive::Box));
    scene.probes.add_probe(Vec3::ZERO);
    let reflection = ReflectionProbe::new(Vec3::ZERO, Vec3::ONE);
    scene.reflection_probes.probes.push(reflection);
    (scene, base)
}

fn mark_id() -> SceneId {
    static ID: OnceLock<SceneId> = OnceLock::new();
    *ID.get_or_init(SceneId::next)
}

macro_rules! marks {
    ($($name:literal $(as $part:ident)? => |$s:ident, $b:pat_param| $set:expr, $is:expr;)*) => {
        &[$(Mark {
            name: $name,
            part: [$(Some(Class::$part),)? None][0],
            set: |$s, $b| { $set; },
            is_set: |$s, $b| $is,
        }),*]
    };
}

/// Every mark, in the inventory's order.
pub const MARKS: &[Mark] = marks! {
    "id" => |s, _| s.restore_id(mark_id()), s.id() == mark_id();
    "world" => |s, _| s.add_entity("Marked".into()), s.find_entity_by_name("Marked").is_some();
    "selected_entity_id" => |s, b| s.selected_entity_id = Some(b), s.selected_entity_id == Some(b);
    "skybox_path" => |s, _| s.skybox_path = "marked.ktx2".into(), s.skybox_path == "marked.ktx2";
    "ambient_color" => |s, _| s.ambient_color = Vec3::new(0.1, 0.2, 0.3),
        s.ambient_color == Vec3::new(0.1, 0.2, 0.3);
    "ambient_intensity" => |s, _| s.ambient_intensity = 1.25, s.ambient_intensity == 1.25;
    "ambient" => |s, _| s.ambient.equator = Vec3::new(0.3, 0.2, 0.1),
        s.ambient.equator == Vec3::new(0.3, 0.2, 0.1);
    "nav_settings" => |s, _| s.nav_settings.agent_radius = 0.75, s.nav_settings.agent_radius == 0.75;
    "fog" => |s, _| s.fog.density = 0.0125, s.fog.density == 0.0125;
    "sky" => |s, _| s.sky.cloud_coverage = 0.375, s.sky.cloud_coverage == 0.375;
    "layers" => |s, _| s.layers.set_name(9, "Marked"), s.layers.name(9) == "Marked";
    "collision_matrix" => |s, _| s.collision_matrix.set_collision(9, 10, false),
        !s.collision_matrix.can_collide(9, 10);
    "materials" => |s, _| s.materials.insert("Marked".into(), Default::default()),
        s.materials.contains_key("Marked");
    "decals" => |s, b| s.spawn_decal(Vec3::ZERO, Vec3::Y, DecalSpec {
            owner: Some(b),
            ..Default::default()
        }),
        s.decals.iter().any(|d| d.owner.is_some_and(|o| o.entity == b));
    "probes" => |s, _| s.probes.move_probe(0, Vec3::new(1.5, 0.0, 0.0)),
        s.probes.probes.first().is_some_and(|p| p.position.x == 1.5);
    "probes.sh" as BakeOutput => |s, _| s.probes.probes[0].sh.coeffs[0] = [0.5; 3],
        s.probes.probes.first().is_some_and(|p| p.sh.coeffs[0] == [0.5; 3]);
    "reflection_probes" => |s, _| s.reflection_probes.probes[0].importance = 3,
        s.reflection_probes.probes.first().is_some_and(|p| p.importance == 3);
    "reflection_probes.cubemap" as BakeOutput
        => |s, _| s.reflection_probes.probes[0].cubemap_path = "marked.ktx2".into(),
        s.reflection_probes.probes.first().is_some_and(|p| p.cubemap_path == "marked.ktx2");
    "lightmaps" => |s, b| s.lightmaps.entries.push(LightmapEntry {
            entity: b,
            page: 0,
            scale_offset: [1.0, 1.0, 0.0, 0.0],
        }),
        s.lightmaps.get(b).is_some();
    "lighting_settings" => |s, _| s.lighting_settings.reflections.budget_mb = 77,
        s.lighting_settings.reflections.budget_mb == 77;
    "pending_destroy" => |s, b| s.pending_destroy.push(b), s.pending_destroy.contains(&b);
    "playing" => |s, _| s.playing = true, s.is_playing();
    "pending_load" => |s, _| s.pending_load = Some("marked.scene".into()), s.pending_load.is_some();
    "persistent" => |s, b| s.persistent.insert(b), s.persistent.contains(&b);
    "shader_overrides" => |s, b| s.shader_overrides.set(b, "marked".into(), vec![1.0]),
        s.shader_overrides.has(b);
    "shader_time" => |s, _| s.shader_time = 7.5, s.shader_time == 7.5;
    "ui_time" => |s, _| s.ui_time = 7.5, s.ui_time == 7.5;
    "meshes_read_at" => |s, _| s.meshes_read_at = u64::MAX - 1, s.meshes_read_at == u64::MAX - 1;
    "meshes_reimported" => |s, _| s.meshes_reimported.insert("marked.glb".into(), 7),
        s.meshes_reimported.contains_key("marked.glb");
    "disk_text" => |s, _| s.disk_text.record("marked.scene", "marked".into()),
        s.is_disk_text("marked.scene", "marked");
    "history" => |s, b| {
            s.begin_edits();
            if let Some(mut t) = s.world.transform_mut(b) {
                t.position.y = 2.0;
            }
            s.commit_edits(Some("Marked"))
        },
        s.undo_name() == Some("Marked") && s.world.contains(b);
};

/// The dev build's marks.
#[cfg(feature = "dev")]
pub const DEV_MARKS: &[Mark] = marks! {
    "screen_exposure" => |s, _| s.screen_exposure = Some(7.5), s.screen_exposure == Some(7.5);
    "lighting_bake_running" => |s, _| s.lighting_bake_running = true, s.lighting_bake_running;
};

/// Every mark in this build.
pub fn all() -> Vec<&'static Mark> {
    let marks = MARKS.iter();
    #[cfg(feature = "dev")]
    let marks = marks.chain(DEV_MARKS);
    marks.collect()
}
