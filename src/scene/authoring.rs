//! src/scene/authoring.rs — Shared structural-authoring verbs.
//!
//! The one place the engine knows how to change scene *structure*: create an
//! entity (optionally as one of the hierarchy toolbar's primitives), attach or
//! detach a first-class component with the inspector's "Add Component" defaults,
//! and the primitive-mesh geometry shared with serialization's rehydrate path.
//!
//! BOTH callers route through here so editor and API can never drift: the editor
//! (`hierarchy.rs` create, `inspector_add.rs` add-menu) and the API surface
//! (`api::scene`), which the console REPL and headless session drive against the
//! live edit world. Parenting/destroy/save have their own canonical `Scene` entry
//! points (`set_parent` / `destroy_entity` / `save_to_file`); the structural API
//! verbs call straight into those, so they are not duplicated here.
//!
//! Allowed deps: components, render::mesh (primitive geometry), scene.

use glam::Vec3;

use crate::render::mesh as primitives;
use crate::render::mesh::Vertex;
use crate::scene::authoring_defaults::light;
use crate::scene::{LightComponent, LightType, MeshComponent, ParticleEmitterComponent, Scene};

// Re-export the per-component defaults so callers (editor add-menu, `Scene.AddComponent`
// API) reach them through this one entry point; they live in `authoring_defaults`
// only to keep this module under the size cap.
pub use crate::scene::authoring_defaults::{
    attach_default_material, default_animator, default_camera, default_collider, default_health,
    default_light, default_material, default_nav_agent, default_rigidbody,
    default_visual_correction, material_asset_from_import,
};

/// The primitive set the hierarchy toolbar's "Create" dropdown offers — meshes
/// and lights. A `None` primitive (or an unrecognised name) creates a bare entity
/// carrying only its mandatory `Transform`, matching the toolbar's behaviour for
/// a name with no primitive picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Box,
    Sphere,
    Plane,
    Cylinder,
    PointLight,
    DirectionalLight,
    SpotLight,
}

impl Primitive {
    /// Parse a toolbar/API primitive name (case-insensitive). Returns `None` for
    /// an empty/unknown name, so the caller creates a bare entity.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "box" => Some(Self::Box),
            "sphere" => Some(Self::Sphere),
            "plane" => Some(Self::Plane),
            "cylinder" => Some(Self::Cylinder),
            "pointlight" => Some(Self::PointLight),
            "directionallight" => Some(Self::DirectionalLight),
            "spotlight" => Some(Self::SpotLight),
            _ => None,
        }
    }
}

/// The geometry for a primitive mesh (`Box`/`Sphere`/`Plane`/`Cylinder`). This is
/// the single source of truth for primitive shapes, reused by serialization's
/// `rehydrate_one` so a created primitive and a loaded one are byte-identical.
/// Returns `None` for the light primitives, which carry no mesh.
pub fn primitive_geometry(primitive: Primitive) -> Option<(Vec<Vertex>, Vec<u32>)> {
    match primitive {
        Primitive::Box => Some(primitives::generate_box(1.0, 1.0, 1.0)),
        Primitive::Sphere => Some(primitives::generate_sphere(1.0, 16, 16)),
        Primitive::Plane => Some(primitives::generate_plane(15.0, 15.0)),
        Primitive::Cylinder => Some(primitives::generate_cylinder(
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
            0.5,
            12,
        )),
        Primitive::PointLight | Primitive::DirectionalLight | Primitive::SpotLight => None,
    }
}

/// The canonical primitive-mesh name (matches `MeshComponent::primitive_type` and
/// the `rehydrate_one` keys), or `None` for the light primitives.
fn primitive_mesh_name(primitive: Primitive) -> Option<&'static str> {
    match primitive {
        Primitive::Box => Some("Box"),
        Primitive::Sphere => Some("Sphere"),
        Primitive::Plane => Some("Plane"),
        Primitive::Cylinder => Some("Cylinder"),
        _ => None,
    }
}

/// Build the `MeshComponent` for a mesh primitive (geometry + the on-disk
/// `primitive_type` reference). Returns `None` for the light primitives.
pub fn primitive_mesh_component(primitive: Primitive) -> Option<MeshComponent> {
    let name = primitive_mesh_name(primitive)?;
    let (vertices, indices) = primitive_geometry(primitive)?;
    Some(MeshComponent {
        primitive_type: name.to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: crate::scene::DirtyFlag::new(true),
    })
}

/// The default light for a light primitive (the hierarchy toolbar's values).
fn primitive_light(primitive: Primitive) -> Option<LightComponent> {
    match primitive {
        Primitive::PointLight => Some(light(LightType::Point, Vec3::ONE, 1.5, 10.0)),
        Primitive::DirectionalLight => Some(light(
            LightType::Directional,
            Vec3::new(1.0, 0.95, 0.8),
            2.0,
            0.0,
        )),
        Primitive::SpotLight => Some(light(LightType::Spotlight, Vec3::ONE, 2.0, 15.0)),
        _ => None,
    }
}

/// Create an entity named `name`, configured for `primitive` (a mesh primitive
/// gets a `MeshComponent`, a light primitive a `LightComponent`, `None` a bare
/// transform-only entity). Returns the new entity's stable id. This is the path
/// the hierarchy toolbar's "Create" button and the `Scene.CreateEntity` API both
/// call, so editor and API behaviour match exactly.
pub fn create_entity(scene: &mut Scene, name: &str, primitive: Option<Primitive>) -> u32 {
    let id = scene.add_entity(name.to_string());
    if let Some(primitive) = primitive {
        if let Some(mut entity) = scene.get_entity_mut(id) {
            if let Some(mesh) = primitive_mesh_component(primitive) {
                entity.mesh = Some(mesh);
            } else if let Some(light) = primitive_light(primitive) {
                entity.light = Some(light);
            }
        }
    }
    id
}

/// The first-class components the inspector's "Add Component" menu can attach /
/// detach. `Script` is excluded: a script attachment carries a path (it is "add
/// *which* script"), so it has its own API verb rather than a defaulted add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentKind {
    Light,
    Health,
    Animator,
    Collider,
    RigidBody,
    Texture,
    NavMeshAgent,
    Camera,
    Particles,
    VisualCorrection,
}

impl ComponentKind {
    /// Parse an "Add Component" kind name (case-insensitive). Accepts the short
    /// names the menu uses (e.g. `RigidBody`, `Texture`, `NavMeshAgent`).
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "light" => Some(Self::Light),
            "health" => Some(Self::Health),
            "animator" => Some(Self::Animator),
            "collider" => Some(Self::Collider),
            "rigidbody" => Some(Self::RigidBody),
            "texture" | "material" => Some(Self::Texture),
            "navmeshagent" | "navagent" => Some(Self::NavMeshAgent),
            "camera" => Some(Self::Camera),
            "particles" | "particlesystem" => Some(Self::Particles),
            "visualcorrection" => Some(Self::VisualCorrection),
            _ => None,
        }
    }
}

/// Attach a first-class component of `kind` to entity `id` with the inspector's
/// default values, replacing any existing one (editor offers it only when absent;
/// the API is idempotent-by-replace). Returns `false` if the entity is missing. The
/// inspector add-menu and the `Scene.AddComponent` API both route here.
pub fn add_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    // Material attaches a reference AND creates a shared library asset (it touches
    // `scene.materials`, not just the entity guard), so it routes off on its own.
    if kind == ComponentKind::Texture {
        return attach_default_material(scene, id);
    }
    let Some(mut entity) = scene.get_entity_mut(id) else {
        return false;
    };
    match kind {
        ComponentKind::Light => entity.light = Some(default_light()),
        ComponentKind::Health => entity.health = Some(default_health()),
        ComponentKind::Animator => entity.animator = Some(default_animator()),
        ComponentKind::Collider => entity.collider = Some(default_collider()),
        ComponentKind::RigidBody => entity.rigidbody = Some(default_rigidbody()),
        ComponentKind::Texture => unreachable!("handled above"),
        ComponentKind::NavMeshAgent => entity.nav_agent = Some(default_nav_agent()),
        ComponentKind::Camera => entity.camera = Some(default_camera()),
        ComponentKind::Particles => entity.particles = Some(ParticleEmitterComponent::default()),
        ComponentKind::VisualCorrection => {
            entity.visual_correction = Some(default_visual_correction())
        }
    }
    true
}

/// Detach a first-class component of `kind` from entity `id`, applying the same
/// cascades the editor's inspector enforces: removing `Health` also drops the
/// `Animator` it manages, and removing `Camera` also drops the camera-only
/// `VisualCorrection` stack. Returns `false` if the entity does not exist
/// (clearing an absent component is otherwise a no-op success).
pub fn remove_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    let Some(mut entity) = scene.get_entity_mut(id) else {
        return false;
    };
    match kind {
        ComponentKind::Light => entity.light = None,
        ComponentKind::Health => {
            entity.health = None;
            entity.animator = None;
        }
        ComponentKind::Animator => entity.animator = None,
        ComponentKind::Collider => entity.collider = None,
        ComponentKind::RigidBody => entity.rigidbody = None,
        // Drop only the reference; the shared library material may still be in use.
        ComponentKind::Texture => entity.material = None,
        ComponentKind::NavMeshAgent => entity.nav_agent = None,
        ComponentKind::Camera => {
            entity.camera = None;
            entity.visual_correction = None;
        }
        ComponentKind::Particles => entity.particles = None,
        ComponentKind::VisualCorrection => entity.visual_correction = None,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_entity_configures_per_primitive() {
        let mut scene = Scene::new();
        let bare = create_entity(&mut scene, "Empty", None);
        let boxed = create_entity(&mut scene, "Crate", Some(Primitive::Box));
        let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));

        let e = scene.get_entity(bare).unwrap();
        assert!(
            e.mesh.is_none() && e.light.is_none(),
            "bare = transform only"
        );
        let e = scene.get_entity(boxed).unwrap();
        let mesh = e.mesh.as_ref().expect("box gets a mesh");
        assert_eq!(mesh.primitive_type, "Box");
        assert!(!mesh.vertices.is_empty() && e.light.is_none());
        let e = scene.get_entity(lamp).unwrap();
        assert!(e.mesh.is_none());
        assert_eq!(e.light.as_ref().unwrap().light_type, LightType::Point);
    }

    #[test]
    fn add_remove_component_with_cascades() {
        let mut scene = Scene::new();
        let id = create_entity(&mut scene, "Mob", None);
        assert!(add_component(&mut scene, id, ComponentKind::Health));
        assert!(scene.get_entity(id).unwrap().health.is_some());
        assert!(remove_component(&mut scene, id, ComponentKind::Health));
        assert!(scene.get_entity(id).unwrap().health.is_none());

        // Removing Camera cascades to VisualCorrection (editor parity).
        add_component(&mut scene, id, ComponentKind::Camera);
        add_component(&mut scene, id, ComponentKind::VisualCorrection);
        remove_component(&mut scene, id, ComponentKind::Camera);
        {
            let e = scene.get_entity(id).unwrap();
            assert!(e.camera.is_none() && e.visual_correction.is_none());
        }

        assert!(!add_component(&mut scene, 9999, ComponentKind::Light));
    }

    #[test]
    fn parse_names_are_case_insensitive() {
        assert_eq!(Primitive::parse("BOX"), Some(Primitive::Box));
        let mat = ComponentKind::parse("material");
        assert_eq!(mat, Some(ComponentKind::Texture));
        assert_eq!(ComponentKind::parse("nope"), None);
    }
}
