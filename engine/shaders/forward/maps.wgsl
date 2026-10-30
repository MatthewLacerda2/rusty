// engine/shaders/forward/maps.wgsl — reading a material map's texel as the value the
// forward pass shades with (#1574): what a texture's compression keeps decides
// which channels hold it.

#define_import_path forward::maps

// A tangent-space normal from a normal map's texel: x and y from its red and
// green in [0,1], z rebuilt from them, as Unity's `UnpackNormal` does. A
// compressed normal map (BC5) keeps x and y alone.
fn unpack_normal(texel: vec4<f32>) -> vec3<f32> {
    let xy = texel.xy * 2.0 - 1.0;
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
