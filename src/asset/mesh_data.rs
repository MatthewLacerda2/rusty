//! src/asset/mesh_data.rs — the importer's plain-data mesh types.
//!
//! These are deliberately decoupled from the renderer: an `ImportedAsset` is pure
//! geometry + material values (glam only), never a GPU `Vertex` or a wgpu buffer.
//! The scene layer converts a `MeshVertex` into the render `Vertex` when it
//! rehydrates a mesh, so the importer never depends on wgpu/egui/mlua.

use super::anim_data::AnimationClip;
use glam::{Mat4, Quat, Vec3};

/// One vertex of imported geometry: position, normal, a single UV channel and the
/// optional skin binding. `joint_indices` index into the sub-mesh's [`SkinData`]
/// joints (and thus the GPU bone palette); `joint_weights` are the linear-blend
/// weights. Unskinned vertices (every primitive, and any glTF mesh without a skin)
/// default to the identity binding — bone 0, full weight — so the shader leaves
/// them untouched (#79).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub joint_indices: [u32; 4],
    pub joint_weights: [f32; 4],
}

impl MeshVertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], tex_coords: [f32; 2]) -> Self {
        Self {
            position,
            normal,
            tex_coords,
            joint_indices: [0, 0, 0, 0],
            joint_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    /// Attach a skin binding (glTF `JOINTS_0`/`WEIGHTS_0`) to a vertex.
    pub fn with_skin(mut self, joint_indices: [u32; 4], joint_weights: [f32; 4]) -> Self {
        self.joint_indices = joint_indices;
        self.joint_weights = joint_weights;
        self
    }
}

/// A sub-mesh's skeleton, parsed from a glTF `skin`: one inverse-bind matrix and
/// one bind-pose global (model-space) transform per joint, in `skin.joints` order
/// — the same order the per-vertex `joint_indices` reference.
///
/// At bind pose the skinning matrix for each joint is `bind_global * inverse_bind`,
/// which is the identity by construction (the inverse-bind is the inverse of the
/// joint's global bind transform). Storing both halves rather than only the product
/// keeps the rig data the animation runtime (#80) will pose, while letting the
/// renderer upload the bind-pose palette today (#79).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SkinData {
    /// Inverse bind matrix per joint.
    pub inverse_bind: Vec<Mat4>,
    /// Global (model-space) transform of each joint node at bind time, already
    /// expressed relative to the skinned mesh node so the palette is mesh-local.
    pub bind_global: Vec<Mat4>,
    /// Local bind-pose transform (TRS) of each joint node, in joint order. This is
    /// the rest pose the animation runtime (#80) overrides per channel before
    /// re-composing the joint globals; untouched channels keep these defaults.
    pub local_bind: Vec<JointTransform>,
    /// Parent joint *slot* of each joint (the index into this skin's joint arrays),
    /// or `None` for a joint whose parent is not itself a joint (a skeleton root).
    /// Drives the hierarchy composition when posing.
    pub parents: Vec<Option<usize>>,
    /// glTF node index of each joint, in joint order — the key animation channels
    /// target. Lets the clip importer map a channel's node onto a joint slot.
    pub joint_nodes: Vec<usize>,
    /// Inverse of the skinned mesh node's global bind transform. Left-multiplying a
    /// posed joint global by this re-expresses the palette in mesh-local space, the
    /// same convention `bind_global` already bakes in.
    pub mesh_inverse: Mat4,
}

impl SkinData {
    /// The bind-pose bone palette: `bind_global[j] * inverse_bind[j]` per joint.
    /// Joints are paired in order; a length mismatch is truncated to the shorter.
    pub fn bind_palette(&self) -> Vec<Mat4> {
        self.bind_global
            .iter()
            .zip(self.inverse_bind.iter())
            .map(|(g, ib)| *g * *ib)
            .collect()
    }
}

/// A joint's local transform as decomposed translation / rotation / scale — the
/// form glTF animation channels address (separate T/R/S paths) and the form the
/// keyframe sampler interpolates. Composes to a `Mat4` via [`JointTransform::matrix`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for JointTransform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl JointTransform {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// A material's values, as read from the source file — the asset-layer DTO for the
/// full glTF 2.0 metallic-roughness model. Pure data (no GPU buffers): the scalar
/// factors plus the *resolved file paths* to the texture images. The scene layer
/// converts this into a renderable `MaterialAsset` (see `material_asset_from_import`).
///
/// glTF packs metallic + roughness into ONE `metallicRoughnessTexture` (metallic in
/// B, roughness in G), so [`MaterialData::metallic_roughness_map`] is a single path;
/// the conversion fans it out to the engine's separate metallic/roughness maps.
///
/// Only EXTERNAL texture URIs are resolved to paths here; embedded images
/// (`Source::View`, a data URI / buffer view) are left `None` (a documented
/// follow-up — extracting embedded pixels to disk is out of scope for #203).
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    pub name: String,
    /// Linear RGBA base color factor (glTF `pbrMetallicRoughness.baseColorFactor`
    /// or OBJ `Kd` with alpha 1.0). Defaults to opaque white.
    pub base_color: [f32; 4],
    /// glTF `metallicFactor` (default 1.0 per the glTF spec).
    pub metallic: f32,
    /// glTF `roughnessFactor` (default 1.0 per the glTF spec).
    pub roughness: f32,
    /// Resolved path to the base-color (albedo) texture, or `None`.
    pub base_color_map: Option<String>,
    /// Resolved path to the packed metallic-roughness texture (metallic=B,
    /// roughness=G), or `None`.
    pub metallic_roughness_map: Option<String>,
    /// Resolved path to the normal map, or `None`.
    pub normal_map: Option<String>,
    /// Resolved path to the emissive texture, or `None`.
    pub emissive_map: Option<String>,
    /// glTF `emissiveFactor` (linear RGB, default [0,0,0]).
    pub emissive: [f32; 3],
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            // glTF spec defaults: a material with no factors is fully metallic/rough.
            metallic: 1.0,
            roughness: 1.0,
            base_color_map: None,
            metallic_roughness_map: None,
            normal_map: None,
            emissive_map: None,
            emissive: [0.0, 0.0, 0.0],
        }
    }
}

/// One addressable sub-object of a source file, e.g. `crates.glb::Barrel`.
///
/// `id` is the stable, human-readable handle used in scene references; it is the
/// source mesh's name when present, else a deterministic fallback (`mesh_<n>`).
/// Geometry is plain `MeshVertex` + index data; `material` indexes into the parent
/// `ImportedAsset::materials`, or `None` when the sub-mesh declares no material.
#[derive(Clone, Debug, PartialEq)]
pub struct SubMesh {
    pub id: String,
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
    /// The skeleton bound to this sub-mesh, when the source declared a glTF `skin`.
    /// `None` for static meshes (every `.obj`, and any glTF mesh without a skin);
    /// the per-vertex bindings then stay at their unskinned identity default.
    pub skin: Option<SkinData>,
    /// The animation clips that drive this sub-mesh's skeleton, in document order
    /// (#80). Empty for a static mesh or a skinned mesh the source never animates;
    /// each clip's tracks are keyed by the `skin`'s joint slots.
    pub clips: Vec<AnimationClip>,
}

impl SubMesh {
    /// Axis-aligned bounds of this sub-mesh, or `None` when it has no vertices.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut iter = self.vertices.iter();
        let first = iter.next()?;
        let mut min = Vec3::from_array(first.position);
        let mut max = min;
        for v in iter {
            let p = Vec3::from_array(v.position);
            min = min.min(p);
            max = max.max(p);
        }
        Some((min, max))
    }
}

/// The full result of importing one source file: every addressable sub-mesh plus
/// the shared material table. This is the pure-data product the scene layer and
/// the Content Browser both consume.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedAsset {
    pub sub_meshes: Vec<SubMesh>,
    pub materials: Vec<MaterialData>,
}

impl ImportedAsset {
    /// Look up a sub-mesh by its `id` (the part after `::` in a reference).
    pub fn sub_mesh(&self, id: &str) -> Option<&SubMesh> {
        self.sub_meshes.iter().find(|m| m.id == id)
    }

    /// The ids of every sub-object, in file order — the sub-object map surfaced in
    /// the sidecar and the Content Browser.
    pub fn sub_mesh_ids(&self) -> Vec<String> {
        self.sub_meshes.iter().map(|m| m.id.clone()).collect()
    }
}
