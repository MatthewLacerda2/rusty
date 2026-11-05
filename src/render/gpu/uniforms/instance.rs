//! src/render/gpu/uniforms/instance.rs — what a forward draw reads of the objects it
//! draws (#470), split by who writes it (#1706).
//!
//! An [`ObjectData`] is what the object holds: its world matrix, its light-probe SH
//! (#240) and where its baked lightmap sits (#438). A kept scene writes it into the
//! object's slot of the renderer's persistent object array when it changes
//! (`render::scene::ObjectTable`), never per camera. An [`InstanceData`] is what one
//! camera decides for one object this frame: which object, the reflection probes it
//! blends, its proxy volume and its motion history. A camera packs those in draw
//! order and the vertex stage reads its object through it, so batches stay
//! contiguous and a camera's sort stays free.
//!
//! Only what legitimately differs between copies of one mesh + material lives in
//! either. Everything the fragment shader branches on before sampling a texture stays
//! in the draw's `MaterialData` record, indexed by the per-draw uniform, so those
//! branches stay in uniform control flow. The SH branch samples nothing, so it may
//! vary per object.

/// One object's record, at its slot of the object array (`forward/bindings.wgsl`'s
/// `ObjectData`). Slot 0 holds [`ObjectData::IDENTITY`], which overlays name.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ObjectData {
    pub model_matrix: [f32; 16],
    // When `use_sh == 1`, the shader reconstructs ambient irradiance from these 9 L2
    // SH coefficients — the probe field interpolated at this object's position on the
    // CPU — instead of the flat hemispherical ambient term. Each coefficient is an RGB
    // triple in `xyz` (4th lane unused) so the array stays `vec4`-aligned.
    pub use_sh: u32,
    // The baked lightmap (#438): its atlas page plus one (0 for none) and the
    // scale/offset from the mesh's lightmap UV into that page (`uv * xy + zw`). Per
    // object, so lightmapped copies of a prop still share one draw.
    pub lightmap_page: u32,
    // 1 when the lightmap's direction page is bound (#810): normal maps then
    // reshape the baked light, and its `Baked` lamps add one specular lobe (#1695).
    pub lightmap_directional: u32,
    // 1 when the scene holds a lightmap bake that took this mesh in
    // (`BakeScene::takes_mesh`, #1750): its emission is in the bake, so SSIL
    // gathers none of it, lightmap UVs or not.
    pub in_bake: u32,
    pub lightmap_st: [f32; 4],
    pub sh: [[f32; 4]; 9],
}

impl ObjectData {
    /// An object that adds nothing: identity transform, no probe SH, no lightmap.
    pub(crate) const IDENTITY: Self = Self {
        model_matrix: glam::Mat4::IDENTITY.to_cols_array(),
        use_sh: 0,
        lightmap_page: 0,
        lightmap_directional: 0,
        in_bake: 0,
        lightmap_st: [0.0; 4],
        sh: [[0.0; 4]; 9],
    };
}

/// One instance of a forward draw, read by `instance_index` from the camera's packed
/// array (`forward/bindings.wgsl`'s `InstanceData`).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct InstanceData {
    /// The object's slot in the object array.
    pub object: u32,
    // The resident reflection probes this instance blends (#1584): two indices plus
    // one and their weights, packed by `reflection_probes::pack_blend`; 0 for the sky
    // alone. Picked per object at its world-bounds centre (#1206).
    pub reflection_probe: u32,
    // The Light Probe Proxy Volume it reads (#1672): where it starts in the frame's
    // proxy array (`draw::proxy_volume`), plus one; 0 for none.
    pub proxy_volume: u32,
    // Motion vectors (#1435): where last frame's world matrix sits in the joint
    // array, last frame's palette (a skinned draw's) right after it
    // (`motion::track`); 0 moves the instance with the camera alone (no history, a
    // mesh particle).
    pub motion: u32,
}

impl InstanceData {
    /// An instance of `object` that a camera adds nothing to: the sky's reflection,
    /// no proxy volume, no history.
    pub(crate) const fn of(object: u32) -> Self {
        Self {
            object,
            reflection_probe: 0,
            proxy_volume: 0,
            motion: 0,
        }
    }

    /// The identity object's instance. Overlays bind a one-element array of it and
    /// carry their transform in the per-draw uniform.
    pub(crate) const IDENTITY: Self = Self::of(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_keep_the_shader_layouts_size() {
        // `ObjectData`: a mat4, four u32, a vec4, nine vec4. `InstanceData`: four u32.
        assert_eq!(std::mem::size_of::<ObjectData>(), 64 + 16 + 16 + 9 * 16);
        assert_eq!(std::mem::size_of::<InstanceData>(), 16);
    }
}
