// engine/shaders/sky/air.wgsl — the volumetric fog in front of the sky (#1703):
// `skybox.wgsl` and `sky_gradient.wgsl` import it, so both skies read the
// integrated volume the same way.

#define_import_path sky::air

#import fog::volumetric::{through_volume, volumetric_sky}

// The integrated volumetric fog: group 0's, as the forward pass binds it.
@group(0) @binding(17)
var t_fog_volume: texture_3d<f32>;
@group(0) @binding(18)
var s_fog_volume: sampler;

// `color` behind the whole volumetric fog at the sky fragment whose clip-space
// position is `clip`, where the camera's `vol` (`camera.volumetric`) reads it.
fn through_air(color: vec3<f32>, vol: vec4<f32>, clip: vec4<f32>) -> vec3<f32> {
    return through_volume(color, volumetric_sky(t_fog_volume, s_fog_volume, vol, clip));
}
