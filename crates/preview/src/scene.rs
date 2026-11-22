use std::path::Path;

use glam::Vec3;

use rusty_components::{LightMode, MaterialAsset, MaterialComponent};
use rusty_edit::authoring::{self, Primitive};
use rusty_scene::{LightComponent, LightType, Scene};

use crate::stand_in;

/// In the engine's own content (`engine/`), not the project's `assets/`, so it is
/// never listed as a project asset.
fn suzanne_path() -> String {
    rusty_core::project::engine_path("models/suzanne.obj")
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum PreviewMesh {
        #[default]
        Sphere = "Sphere",
        Cube = "Cube",
        Suzanne = "Suzanne",
        Plane = "Plane",
    }
}

#[derive(Clone, Debug)]
pub enum PreviewSubject {
    Model(String),
    Texture(String),
    /// Drawn with the default material;
    /// `shell::editor::viewport::render_preview_scene` swaps in this shader.
    Shader(String),
    Material(Box<MaterialAsset>),
}

impl PreviewSubject {
    pub fn cache_key(&self) -> String {
        match self {
            PreviewSubject::Model(p) => format!("model:{p}"),
            PreviewSubject::Texture(p) => format!("texture:{p}"),
            PreviewSubject::Shader(p) => format!("shader:{p}"),
            PreviewSubject::Material(m) => format!("material:{m:?}"),
        }
    }
}

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
            // A preview is never baked: its light is drawn live.
            mode: LightMode::Mixed,
            shadow_fade_distance: 0.0,
        }),
    );
}

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

fn spawn_model(scene: &mut Scene, path: &str) {
    let Ok(asset) = rusty_asset::import_and_sync_sidecar(Path::new(path)) else {
        return;
    };
    spawn_fitted(scene, asset.model_bounds(), |scene, origin| {
        vec![authoring::instantiate_model(scene, path, &asset, origin)]
    });
}

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

/// Metres: a unit cube's bounding radius, so a model that size keeps it.
const FIT_RADIUS: f32 = 0.866_025_4;

/// Scales the subject, not the camera, which close to a 1 cm model would clip it
/// with its near plane.
fn fit_scale(min: Vec3, max: Vec3) -> f32 {
    let radius = (max - min).length() * 0.5;
    if radius > f32::EPSILON && radius.is_finite() {
        FIT_RADIUS / radius
    } else {
        1.0
    }
}

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
