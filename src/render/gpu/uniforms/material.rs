//! What a forward draw reads, split as Unity's SRP Batcher splits it (#1616): a small
//! per-draw uniform ([`DrawUniform`], group 1 binding 0, at a dynamic offset) and the
//! draw's material record ([`MaterialData`], one element of the group 1 binding 4
//! storage array), which the per-draw uniform names by index.
//!
//! **Per draw** is what differs between two draws of one material: the draw's own
//! transform (identity for instanced solids, an editor gizmo's placement), where its
//! joint palette starts, and which record it reads. **Per material** is everything the
//! material decides: tint, PBR scalars, map flags, emissive, cutout, the decal and
//! lit flags, and the [`SurfaceUniform`] and [`MappingUniform`] terms.
//!
//! Records are keyed by their packed bytes: every frame each distinct record is stored
//! once, in first-draw order (`render::draw::batch::FrameDraws::pack`), so draws with
//! identical material values share one, and the array is written to the GPU only when
//! its bytes changed since the last upload (`DrawBuffers::upload`).
//!
//! **Adding a material field.** Add it to [`MaterialData`] (spend a `_pad` word, or
//! append a 16-byte run at the end, keeping the size a multiple of 16), mirror it in
//! `MaterialData` in `engine/shaders/forward/bindings.wgsl` at the same offset, set it
//! in `render::draw::uniforms::material_data`, add its row to
//! `uniform_layout_tests::draw`, and read it in WGSL as `material().<field>`. The
//! per-draw uniform and its 256-byte stride are untouched; the record has no fixed cap
//! (a storage array's element is as large as its struct).

/// One draw's own data (#1616): mirrors `DrawUniforms` in `forward/bindings.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawUniform {
    /// The draw's own transform, applied on top of each instance's (#470): identity
    /// for instanced solids, the gizmo's placement for editor overlays.
    pub model_matrix: [f32; 16],
    /// Index of this draw's joint 0 in the frame's bone-palette storage array (#455):
    /// the vertex shader reads `bones[bone_base + joint]`. `0` is the shared identity
    /// matrix every non-skinned draw uses.
    pub bone_base: u32,
    /// The draw's record in the material array.
    pub material: u32,
    pub _pad: [u32; 2],
}

/// One material record (#1616): mirrors `MaterialData` in `forward/bindings.wgsl`,
/// where [`SurfaceUniform`] and [`MappingUniform`] are written out flat.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MaterialData {
    pub color_tint: [f32; 4],
    pub use_texture: u32,
    pub is_lit: u32,
    pub metallic: f32,
    pub roughness: f32,
    // Map flags (#202, #207). When set, the shader samples the matching group(2)
    // texture: metallic/roughness multiply their scalar by the sampled channel
    // (glTF's blue/green at 1, red at 2: a one-channel BC4 upload, #1574);
    // `use_normal_map` perturbs the shading normal in tangent space; `use_emissive_map`
    // modulates the emissive factor.
    pub use_metallic_map: u32,
    pub use_roughness_map: u32,
    pub use_normal_map: u32,
    pub use_emissive_map: u32,
    // Flat emissive factor (#222): rgb glow added on top of the lit colour in
    // `fs_main`. The renderer draws to an HDR target with a bloom bright-pass, so
    // values >1.0 glow automatically. A `vec4` (4th lane unused) to dodge WGSL's
    // `vec3` 16-byte-alignment gotcha.
    pub emissive: [f32; 4],
    // Cutout alpha-test (#242). `use_cutout == 1` makes `fs_main` discard fragments
    // whose final alpha is below `alpha_cutoff` (Cutout materials).
    pub use_cutout: u32,
    pub alpha_cutoff: f32,
    // `1` when the surface folds in the decals of its cluster (#638): a lit, non-
    // transparent surface whose material receives decals.
    pub receive_decals: u32,
    pub _pad: u32,
    // How the surface composites, shadows and transmits (#1523).
    pub surface: SurfaceUniform,
    // Where the material's maps are read (#1573).
    pub mapping: MappingUniform,
}

// The stride is a dynamic-offset alignment, not a cap the record has to fit; a storage
// array's elements sit at their struct's 16-byte-rounded size.
const _: () = assert!(std::mem::size_of::<DrawUniform>() <= 256);
const _: () = assert!(std::mem::size_of::<MaterialData>().is_multiple_of(16));

/// How many 32-bit words a [`MaterialData`] holds.
pub(crate) const MATERIAL_DATA_WORDS: usize = std::mem::size_of::<MaterialData>() / 4;

impl MaterialData {
    /// The record as plain words: the batch key compares these bit-for-bit, so two
    /// draws share a batch only when every material scalar and flag is identical.
    pub(crate) fn words(&self) -> [u32; MATERIAL_DATA_WORDS] {
        let mut words = [0u32; MATERIAL_DATA_WORDS];
        words.copy_from_slice(bytemuck::cast_slice(bytemuck::bytes_of(self)));
        words
    }

    /// The record of `words`, as [`Self::words`] packed it.
    pub(crate) fn from_words(words: &[u32; MATERIAL_DATA_WORDS]) -> Self {
        bytemuck::pod_read_unaligned(bytemuck::cast_slice(words))
    }

    /// A flat, unlit record of `tint`: no maps, cutout or decals. What every editor
    /// line gizmo draws with.
    pub(crate) fn flat(tint: [f32; 4]) -> Self {
        Self {
            color_tint: tint,
            roughness: 0.5,
            surface: SurfaceUniform::default(),
            mapping: MappingUniform::default(),
            ..bytemuck::Zeroable::zeroed()
        }
    }
}

/// A draw's transparent-surface terms (#1523): blending, shadows, refraction,
/// absorption, depth postpass and thin translucency, packed from its material.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SurfaceUniform {
    /// The material's `BlendMode`, by index: 3 (Multiply) tints toward white by alpha.
    pub blend_mode: u32,
    /// 1 when URP's preserve specular applies: the diffuse is premultiplied by alpha
    /// in the shader, the specular is not, and the Fresnel raises the opacity.
    pub premultiply_diffuse: u32,
    /// 0 when no shadow lands on the surface (URP's *Receive Shadows* off).
    pub receive_shadows: u32,
    /// The refraction model: 0 none, 1 planar (HDRP's Box), 2 thin.
    pub refraction_model: u32,
    pub ior: f32,
    /// The refraction model's thickness in metres (thin: HDRP's fixed 5 mm).
    pub thickness: f32,
    /// 1 when the surface composites over the opaque copy itself (refraction or
    /// absorption), so it draws with alpha 1.
    pub uses_opaque: u32,
    /// 1 when the surface writes its depth after drawing.
    pub depth_postpass: u32,
    /// The absorption coefficient per metre, rgb (HDRP's
    /// `TransmittanceColorAtDistanceToAbsorption`); `w` the material's thickness,
    /// the depth it absorbs over where the sky is behind it.
    pub absorption: [f32; 4],
    /// Thin translucency: rgb the transmission colour times its amount, `w` the
    /// amount (0: none).
    pub transmission: [f32; 4],
    /// 1 when the surface traces screen-space reflections (#1570): a lit
    /// `Transparent` material with `receive_ssr` that can be smooth enough to
    /// reflect (`postfx::effects::ssr::transparent::traces`); it traces only where
    /// the camera's `stack.z` gives a march.
    pub receive_ssr: u32,
    pub _pad: [u32; 3],
}

/// Where a draw's material maps are read (#1573), packed from its
/// tiling, offset and UV mapping; the default is UV0 untouched, for draws with no material.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MappingUniform {
    /// `xy` the tiling, `zw` the offset (Unity's `_MainTex_ST`).
    pub uv_st: [f32; 4],
    /// 0 UV0, 1 planar (world XZ), 2 triplanar.
    pub uv_mapping: u32,
    /// Texture repeats per metre under a world mapping: `1 / world_size`.
    pub world_scale: f32,
    /// The power the normal is raised to before it weights the three projections.
    pub blend_sharpness: f32,
    pub _pad: u32,
}

impl Default for MappingUniform {
    fn default() -> Self {
        Self {
            uv_st: [1.0, 1.0, 0.0, 0.0],
            uv_mapping: 0,
            world_scale: 1.0,
            blend_sharpness: 4.0,
            _pad: 0,
        }
    }
}
