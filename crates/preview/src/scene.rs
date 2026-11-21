//! crates/preview/src/scene.rs — the isolated preview scene (#352).
//!
//! [`build_preview_scene`] is the single entry point: given a mesh choice and a
//! preview subject, it returns a brand-new [`Scene`] — never the active editor
//! World — carrying the preview mesh (or the model itself), a hardcoded light,
//! and an empty `skybox_path` so the renderer's existing panorama/gradient fallback
//! (`render/ibl/skybox.rs`) draws a procedural sky with no new baking step. The
//! scene is thrown away and rebuilt whenever the subject or mesh choice changes
//! (see `EditorUi::preview_cache`) — it is never assigned into `World::scene`, never
//! serialized, and never touches Play-mode's snapshot/restore.
//!
//! Both front-ends build the scene through here: the Inspector's Preview tab and the
//! headless `Debug.Preview` capture (`dev/preview.rs`, #353). Because it is a plain
//! scene builder with no egui/GPU types, the agent-facing path gets the *same* mesh,
//! light and sky the human sees — the "one preview machinery" the issue asks for.

use std::path::Path;

use glam::Vec3;

use rusty_components::{LightMode, MaterialAsset, MaterialComponent};
use rusty_edit::authoring::{self, Primitive};
use rusty_scene::{LightComponent, LightType, Scene};

use crate::stand_in;

/// The bundled preview mesh (#352): a hardcoded `.obj`, never a project asset — it
/// lives in the engine's own content (`engine/`, found from any project), never in
/// the project's `assets/`, so it is never listed as a project asset.
fn suzanne_path() -> String {
    rusty_core::project::engine_path("models/suzanne.obj")
}

rusty_core::named_enum! {
    /// Which mesh the Preview tab shows the subject on (models render as themselves
    /// instead — see [`PreviewSubject::Model`]); its name is the tab's label.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum PreviewMesh {
        #[default]
        Sphere = "Sphere",
        Cube = "Cube",
        Suzanne = "Suzanne",
        /// A flat square seen from a low angle under a raking light: the view a
        /// tiling wall or floor is judged at ([`crate::stand_in`]).
        Plane = "Plane",
    }
}

/// What the Preview tab is showing — one per the issue's four asset kinds. Each
/// carries the identity needed to (re)build the isolated scene and, for `Model`, to
/// key the rebuild cache (`cache_key`).
#[derive(Clone, Debug)]
pub enum PreviewSubject {
    /// A model source file (`gltf`/`glb`/`obj`) — instantiated whole as its node
    /// tree (#1083), each mesh with its own materials, replacing the preview mesh
    /// entirely.
    Model(String),
    /// A texture file, applied as the preview mesh's base-color map on the default
    /// material — "how would this map look on a surface" (#352).
    Texture(String),
    /// A `.wgsl` shader file: the preview mesh with default material maps, shaded by
    /// this module. The scene itself carries only the default material; the render
    /// call swaps the pipeline's shader (see `shell::editor::viewport::render_preview_scene`).
    Shader(String),
    /// An already-resolved material asset — from the entity's `MaterialComponent`
    /// card, or looked up by name by `Debug.PreviewMaterial` (#404) — since materials
    /// have no on-disk file to dispatch on.
    Material(Box<MaterialAsset>),
}

impl PreviewSubject {
    /// A stable identity string for the rebuild cache: subject kind + path (or the
    /// material's base-color map / a value fingerprint for `Material`, since a
    /// library material can be edited in place without changing its key).
    pub fn cache_key(&self) -> String {
        match self {
            PreviewSubject::Model(p) => format!("model:{p}"),
            PreviewSubject::Texture(p) => format!("texture:{p}"),
            PreviewSubject::Shader(p) => format!("shader:{p}"),
            PreviewSubject::Material(m) => format!("material:{m:?}"),
        }
    }
}

/// Build a fresh, isolated preview [`Scene`]: empty skybox path (gradient sky, #352),
/// a hardcoded directional light (raking across the plane, a three-quarter key on
/// every other stand-in), and the requested subject, its material drawn `repeats`
/// times across the stand-in, or as authored for `None` ([`stand_in::repeated`],
/// whose refusal of a count the renderer cannot show is the error). A model ignores
/// `mesh` and `repeats`. Never mutates or reads the active editor scene.
pub fn build_preview_scene(
    mesh: PreviewMesh,
    repeats: Option<u32>,
    subject: &PreviewSubject,
) -> Result<Scene, String> {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();

    add_key_light(&mut scene, stand_in::stand_in(mesh, subject));
    let repeated = |material| stand_in::repeated(material, mesh, repeats);

    match subject {
        PreviewSubject::Model(path) => spawn_model(&mut scene, path),
        PreviewSubject::Texture(path) => {
            let id = spawn_preview_mesh(&mut scene, mesh);
            let material = MaterialAsset {
                base_color_map: Some(path.as_str().into()),
                ..MaterialAsset::default()
            };
            attach_material(&mut scene, id, repeated(material)?);
        }
        PreviewSubject::Shader(_) => {
            let id = spawn_preview_mesh(&mut scene, mesh);
            attach_material(&mut scene, id, repeated(MaterialAsset::default())?);
        }
        PreviewSubject::Material(material) => {
            let id = spawn_preview_mesh(&mut scene, mesh);
            attach_material(&mut scene, id, repeated((**material).clone())?);
        }
    }

    Ok(scene)
}

/// A single hardcoded directional light, posed for what is `shown`
/// ([`stand_in::light_pose`]).
fn add_key_light(scene: &mut Scene, shown: Option<PreviewMesh>) {
    let id = scene.add_entity("PreviewLight".to_string());
    if let Some(mut transform) = scene.world.transform_mut(id) {
        transform.rotation = stand_in::light_pose(shown);
    }
    scene.world.set_light(
        id,
        Some(LightComponent {
            area_size: glam::Vec2::ONE,
            light_type: LightType::Directional,
            color: Vec3::new(1.0, 0.95, 0.85),
            intensity: 2.0,
            range: 0.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            cast_shadows: false,
            // A preview is never baked: drawn live, as before `Baked` was the default.
            mode: LightMode::Mixed,
            shadow_fade_distance: 0.0,
        }),
    );
}

/// Spawn the stand-in, returning its entity id: the sphere, cube or embedded Suzanne
/// at the origin, where the default orbit looks, or the plane at
/// [`stand_in::PLANE_CENTRE`], past the origin its own framing looks at.
fn spawn_preview_mesh(scene: &mut Scene, mesh: PreviewMesh) -> u32 {
    match mesh {
        PreviewMesh::Sphere => {
            authoring::create_drawn_entity(scene, "PreviewMesh", Some(Primitive::Sphere))
        }
        PreviewMesh::Cube => {
            authoring::create_drawn_entity(scene, "PreviewMesh", Some(Primitive::Box))
        }
        PreviewMesh::Suzanne => spawn_suzanne(scene),
        PreviewMesh::Plane => spawn_plane(scene),
    }
}

/// The stock `Plane` primitive, scaled to [`stand_in::PLANE_SIZE`] and placed at
/// [`stand_in::PLANE_CENTRE`]; its UVs span it once, as a quad's do.
fn spawn_plane(scene: &mut Scene) -> u32 {
    let id = authoring::create_drawn_entity(scene, "PreviewMesh", Some(Primitive::Plane));
    let native = plane_primitive_edge();
    if let Some(mut transform) = scene.world.transform_mut(id) {
        let edge = stand_in::PLANE_SIZE / native;
        transform.position = stand_in::PLANE_CENTRE;
        transform.scale = Vec3::new(edge, 1.0, edge);
    }
    id
}

/// The stock `Plane` primitive's own edge in metres, read off its geometry so the
/// preview follows the primitive if its size ever changes.
pub(crate) fn plane_primitive_edge() -> f32 {
    let (vertices, _) =
        rusty_scene::create::primitive_geometry(Primitive::Plane).unwrap_or_default();
    let xs = vertices.iter().map(|v| v.position[0]);
    let (min, max) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    if max > min {
        max - min
    } else {
        1.0
    }
}

/// Instantiate the embedded Suzanne `.obj` exactly like any other model asset — same
/// `rusty_asset::import_and_sync_sidecar` + `authoring::instantiate_imported` path the model
/// inspector's "Instantiate into Scene" button uses (#352 leans on existing plumbing
/// rather than a bespoke embedded-mesh loader) — then fitted like a model subject.
fn spawn_suzanne(scene: &mut Scene) -> u32 {
    let fallback = |scene: &mut Scene| {
        authoring::create_drawn_entity(scene, "PreviewMesh", Some(Primitive::Sphere))
    };
    let Ok(asset) = rusty_asset::import_and_sync_sidecar(Path::new(&suzanne_path())) else {
        return fallback(scene);
    };
    let Some(sub) = asset.sub_meshes.first() else {
        return fallback(scene);
    };
    let path = suzanne_path();
    let ids = spawn_fitted(scene, sub.bounds(), |scene, origin| {
        authoring::instantiate_imported(scene, &path, &asset, &sub.id, Some("PreviewMesh"), origin)
            .into_iter()
            .collect()
    });
    ids.first().copied().unwrap_or_else(|| fallback(scene))
}

/// Instantiate the model at `path` as its node tree (#1083), fitted to the frame and
/// carrying its own imported materials. Unlike the model inspector's "Instantiate into Scene", which
/// spawns at the file's own size, the preview rescales it (see [`fit_scale`]).
fn spawn_model(scene: &mut Scene, path: &str) {
    let Ok(asset) = rusty_asset::import_and_sync_sidecar(Path::new(path)) else {
        return;
    };
    spawn_fitted(scene, asset.model_bounds(), |scene, origin| {
        vec![authoring::instantiate_model(scene, path, &asset, origin)]
    });
}

/// Centre and fit: `spawn` instantiates the subject whose bounds (spawned at the
/// origin) are `bounds` at the origin it is handed, and every entity it returns is
/// scaled by [`fit_scale`], so the subject lands centred on the world origin at the
/// fitted size. A whole model is one root, so its parts keep their layout. Returns
/// `spawn`'s ids.
///
/// The preview camera always orbits the **world origin**, so a subject whose vertices
/// were authored somewhere else renders as empty sky — the mesh is in the scene, just
/// never in shot. That is not a rare edge case: the engine's own bundled Suzanne sits
/// around `(-2.5, 1.3, 4.1)`, and exported models routinely carry whatever offset they
/// had in the authoring tool. Spawning at `-centre` puts the subject where the camera
/// is already looking, for the Inspector tab and `Debug.Preview` alike.
fn spawn_fitted(
    scene: &mut Scene,
    bounds: Option<(Vec3, Vec3)>,
    spawn: impl FnOnce(&mut Scene, Vec3) -> Vec<u32>,
) -> Vec<u32> {
    let (centre, scale) = bounds.map_or((Vec3::ZERO, 1.0), |(min, max)| {
        ((min + max) * 0.5, fit_scale(min, max))
    });
    let ids = spawn(scene, -centre * scale);
    for &id in &ids {
        if let Some(mut transform) = scene.world.transform_mut(id) {
            transform.scale *= scale;
        }
    }
    ids
}

/// The bounding radius a model is fitted to: a unit cube's, the size of the stock
/// sphere and cube, so a model that size previews at its own size.
const FIT_RADIUS: f32 = 0.866_025_4;

/// The uniform scale that fits bounds `min..max` to [`FIT_RADIUS`] (#1280).
///
/// The camera sits at a fixed distance, so a model far from a metre across renders
/// as a speck or overflows the frame: a 1 cm FBX quad (the file's default unit is
/// the centimetre) was a sub-pixel dot. Unity's asset preview frames by bounds; this
/// scales the subject instead of moving the camera, since a camera close enough
/// for a centimetre would sit inside its near plane and the shot stays fixed.
/// Flat in every axis (a point, or no geometry) keeps its size.
fn fit_scale(min: Vec3, max: Vec3) -> f32 {
    let radius = (max - min).length() * 0.5;
    if radius > f32::EPSILON && radius.is_finite() {
        FIT_RADIUS / radius
    } else {
        1.0
    }
}

/// Insert `material` under a scene-local key and reference it from `id`. The
/// preview scene is thrown away every rebuild, so the key only has to be unique
/// within this one scene.
fn attach_material(scene: &mut Scene, id: u32, material: MaterialAsset) {
    let key = "preview::material".to_string();
    scene.materials.insert(key.clone(), material);
    scene
        .world
        .set_material(id, Some(MaterialComponent::new(key)));
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod scene_tests;
