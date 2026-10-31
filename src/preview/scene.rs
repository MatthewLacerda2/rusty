//! src/preview/scene.rs — the isolated preview scene (#352).
//!
//! [`build_preview_scene`] is the single entry point: given a mesh choice and a
//! preview subject, it returns a brand-new [`Scene`] — never the active editor
//! World — carrying the preview mesh (or the model itself), a hardcoded key light,
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

use glam::{Quat, Vec3};

use crate::asset;
use crate::components::{MaterialAsset, MaterialComponent};
use crate::scene::authoring::{self, Primitive};
use crate::scene::{LightComponent, LightType, Scene};

/// The bundled preview mesh (#352): a hardcoded `.obj`, never a project asset — it
/// lives in the engine's own content (`engine/`, found from any project), never in
/// the project's `assets/` the Content Browser scans, so it is never browsable.
fn suzanne_path() -> String {
    crate::core::project::engine_path("models/suzanne.obj")
}

crate::core::named_enum! {
    /// Which mesh the Preview tab shows the subject on (models render as themselves
    /// instead — see [`PreviewSubject::Model`]); its name is the tab's label.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum PreviewMesh {
        #[default]
        Sphere = "Sphere",
        Cube = "Cube",
        Suzanne = "Suzanne",
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
    /// call swaps the pipeline's shader (see `render_preview_scene` in `main.rs`).
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
/// a hardcoded directional key light, and the requested subject. Never mutates or
/// reads the active editor scene.
pub fn build_preview_scene(mesh: PreviewMesh, subject: &PreviewSubject) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();

    add_key_light(&mut scene);

    match subject {
        PreviewSubject::Model(path) => spawn_model(&mut scene, path),
        PreviewSubject::Texture(path) => {
            let id = spawn_preview_mesh(&mut scene, mesh);
            let material = MaterialAsset {
                base_color_map: Some(path.as_str().into()),
                ..MaterialAsset::default()
            };
            attach_material(&mut scene, id, material);
        }
        PreviewSubject::Shader(_) => {
            let id = spawn_preview_mesh(&mut scene, mesh);
            attach_material(&mut scene, id, MaterialAsset::default());
        }
        PreviewSubject::Material(material) => {
            let id = spawn_preview_mesh(&mut scene, mesh);
            attach_material(&mut scene, id, (**material).clone());
        }
    }

    scene
}

/// A single hardcoded directional light, angled like a classic 3/4 key light.
fn add_key_light(scene: &mut Scene) {
    let id = scene.add_entity("PreviewLight".to_string());
    if let Some(mut transform) = scene.world.transform_mut(id) {
        transform.rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            (-40.0f32).to_radians(),
            (-35.0f32).to_radians(),
            0.0,
        );
    }
    scene.world.set_light(
        id,
        Some(LightComponent {
            light_type: LightType::Directional,
            color: Vec3::new(1.0, 0.95, 0.85),
            intensity: 2.0,
            range: 0.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            cast_shadows: false,
            mode: Default::default(),
        }),
    );
}

/// Spawn the sphere/cube primitive or the embedded Suzanne mesh at the origin
/// (where the orbit camera always looks), returning its entity id.
fn spawn_preview_mesh(scene: &mut Scene, mesh: PreviewMesh) -> u32 {
    match mesh {
        PreviewMesh::Sphere => {
            authoring::create_entity(scene, "PreviewMesh", Some(Primitive::Sphere))
        }
        PreviewMesh::Cube => authoring::create_entity(scene, "PreviewMesh", Some(Primitive::Box)),
        PreviewMesh::Suzanne => spawn_suzanne(scene),
    }
}

/// Instantiate the embedded Suzanne `.obj` exactly like any other model asset — same
/// `asset::import_and_sync_sidecar` + `authoring::instantiate_imported` path the model
/// inspector's "Instantiate into Scene" button uses (#352 leans on existing plumbing
/// rather than a bespoke embedded-mesh loader) — then fitted like a model subject.
fn spawn_suzanne(scene: &mut Scene) -> u32 {
    let fallback =
        |scene: &mut Scene| authoring::create_entity(scene, "PreviewMesh", Some(Primitive::Sphere));
    let Ok(asset) = asset::import_and_sync_sidecar(Path::new(&suzanne_path())) else {
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
    let Ok(asset) = asset::import_and_sync_sidecar(Path::new(path)) else {
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
