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
/// lives in the engine's own `assets/` tree (unlike `project/assets/`, which is all
/// the Content Browser scans), so it never shows up as a browsable asset.
pub const SUZANNE_PATH: &str = "assets/models/suzanne.obj";

/// Which mesh the Preview tab shows the subject on (models render as themselves
/// instead — see [`PreviewSubject::Model`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewMesh {
    #[default]
    Sphere,
    Cube,
    Suzanne,
}

impl PreviewMesh {
    pub const ALL: [PreviewMesh; 3] =
        [PreviewMesh::Sphere, PreviewMesh::Cube, PreviewMesh::Suzanne];

    pub fn label(self) -> &'static str {
        match self {
            PreviewMesh::Sphere => "Sphere",
            PreviewMesh::Cube => "Cube",
            PreviewMesh::Suzanne => "Suzanne",
        }
    }
}

/// What the Preview tab is showing — one per the issue's four asset kinds. Each
/// carries the identity needed to (re)build the isolated scene and, for `Model`, to
/// key the rebuild cache (`cache_key`).
#[derive(Clone, Debug)]
pub enum PreviewSubject {
    /// A model source file (`gltf`/`glb`/`obj`) — every sub-object is instantiated
    /// with its own materials, replacing the preview mesh entirely.
    Model(String),
    /// A texture file, applied as the preview mesh's base-color map on the default
    /// material — "how would this map look on a surface" (#352).
    Texture(String),
    /// A `.wgsl` shader file: the preview mesh with default material maps, shaded by
    /// this module. The scene itself carries only the default material; the render
    /// call swaps the pipeline's shader (see `render_preview_scene` in `main.rs`).
    Shader(String),
    /// An already-resolved material asset (from the entity's `MaterialComponent`
    /// card, since materials have no on-disk file to dispatch on).
    Material(MaterialAsset),
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
                base_color_map: Some(path.clone()),
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
            attach_material(&mut scene, id, material.clone());
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
/// `asset::import_and_sync_sidecar` + `authoring::instantiate_asset` path the model
/// inspector's "Instantiate into Scene" button uses (#352 leans on existing plumbing
/// rather than a bespoke embedded-mesh loader).
fn spawn_suzanne(scene: &mut Scene) -> u32 {
    let asset = match asset::import_and_sync_sidecar(Path::new(SUZANNE_PATH)) {
        Ok(a) => a,
        Err(_) => return authoring::create_entity(scene, "PreviewMesh", Some(Primitive::Sphere)),
    };
    let Some(sub) = asset.sub_meshes.first() else {
        return authoring::create_entity(scene, "PreviewMesh", Some(Primitive::Sphere));
    };
    let reference = format!("{SUZANNE_PATH}{}{}", asset::REF_SEPARATOR, sub.id);
    authoring::instantiate_asset(
        scene,
        &reference,
        Some("PreviewMesh"),
        -bounds_centre(&asset.sub_meshes),
    )
    .unwrap_or_else(|_| authoring::create_entity(scene, "PreviewMesh", Some(Primitive::Sphere)))
}

/// Instantiate every sub-object of `path`, centred on the origin and carrying its own
/// imported materials — mirrors `inspector/assets/model.rs`'s "Instantiate into Scene".
/// All sub-objects share one offset, so a multi-part model keeps its internal layout.
fn spawn_model(scene: &mut Scene, path: &str) {
    let Ok(asset) = asset::import_and_sync_sidecar(Path::new(path)) else {
        return;
    };
    let origin = -bounds_centre(&asset.sub_meshes);
    for sub in &asset.sub_meshes {
        let reference = format!("{path}{}{}", asset::REF_SEPARATOR, sub.id);
        let _ = authoring::instantiate_asset(scene, &reference, Some(&sub.id), origin);
    }
}

/// The centre of the union of `subs`' local bounds (`Vec3::ZERO` when they have no
/// geometry).
///
/// The preview camera always orbits the **world origin**, so a subject whose vertices
/// were authored somewhere else renders as empty sky — the mesh is in the scene, just
/// never in shot. That is not a rare edge case: the engine's own bundled Suzanne sits
/// around `(-2.5, 1.3, 4.1)`, and exported models routinely carry whatever offset they
/// had in the authoring tool. Spawning at `-centre` puts the subject where the camera
/// is already looking, for the Inspector tab and `Debug.Preview` alike.
fn bounds_centre(subs: &[asset::SubMesh]) -> Vec3 {
    let mut bounds: Option<(Vec3, Vec3)> = None;
    for sub in subs {
        let Some((lo, hi)) = sub.bounds() else {
            continue;
        };
        bounds = Some(match bounds {
            Some((min, max)) => (min.min(lo), max.max(hi)),
            None => (lo, hi),
        });
    }
    bounds.map_or(Vec3::ZERO, |(min, max)| (min + max) * 0.5)
}

/// Insert `material` under a scene-local key and reference it from `id`. The
/// preview scene is thrown away every rebuild, so the key only has to be unique
/// within this one scene.
fn attach_material(scene: &mut Scene, id: u32, material: MaterialAsset) {
    let key = "preview::material".to_string();
    scene.materials.insert(key.clone(), material);
    scene
        .world
        .set_material(id, Some(MaterialComponent { material: key }));
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod scene_tests;
