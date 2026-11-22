// engine/shaders/forward/maps.wgsl — reading a material map's texel as the value the
// forward pass shades with: what a texture's compression keeps decides
// which channels hold it.

#define_import_path forward::maps

// A normal map's texel as the vector it stores: red, green and blue in [0,1] mapped
// to [-1,1], or as they are where the upload stored them signed (a 16-bit map).
fn stored_normal(texel: vec4<f32>, signed: bool) -> vec3<f32> {
    return select(texel.xyz * 2.0 - 1.0, texel.xyz, signed);
}

// A tangent-space normal from a stored vector: its x and y, z rebuilt from them, as
// Unity's `UnpackNormal` does. A compressed normal map (BC5) keeps x and y alone.
fn unpack_normal(stored: vec3<f32>) -> vec3<f32> {
    let xy = stored.xy;
    return vec3<f32>(xy, sqrt(max(1.0 - dot(xy, xy), 0.0)));
}

// The factor a metallic or roughness map multiplies its scalar by, under its
// entity flag: 1 when the flag is 0 (no map), the texel's `packed` channel
// (glTF's blue for metallic, green for roughness) at 1, and its red at 2 (a
// one-channel BC4 upload, whose value is in red alone).
fn map_factor(texel: vec4<f32>, flag: u32, packed: u32) -> f32 {
    let value = select(texel[packed], texel.r, flag == 2u);
    return select(1.0, value, flag != 0u);
}
