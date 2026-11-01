// engine/shaders/lighting/ambient.wgsl — the scene's ambient light as the shaders
// read it (#1647): the `Color` hemisphere, the `Gradient` trilight or the `Skybox`
// sky's SH (#1675), the WGSL side of `scene::settings::ambient`. The data is `common::AmbientLight` (in
// `common::LightingUniforms`); the forward pass (`forward::lighting::flat_ambient`)
// and lit particles evaluate it here.

#define_import_path lighting::ambient

#import common::AmbientLight
#import lighting::sh::sh_basis

// The ambient a surface with unit normal `n` takes, intensity applied, as
// irradiance / pi. `Color` mode is the hemisphere the forward pass has always
// drawn, `color` straight up fading to a quarter of it straight down; `Gradient` is
// Unity's trilight, linear in `n.y` from the ground through the equator to the sky;
// `Skybox` is the sky's L2 SH, uploaded already convolved. The CPU's
// `scene::AmbientRadiance::at` is the same function (a GPU test holds the two equal); lightmaps and probes bake its
// `incoming` radiance, whose cosine average over a hemisphere is this again.
fn ambient_radiance(ambient: AmbientLight, n: vec3<f32>) -> vec3<f32> {
    if (ambient.mode > 1.5) {
        return sky_sh(ambient, n);
    }
    if (ambient.mode > 0.5) {
        let toward = select(ambient.gradient_ground, ambient.gradient_sky, n.y >= 0.0);
        return mix(ambient.gradient_equator, toward, abs(n.y)) * ambient.intensity;
    }
    let ground_color = ambient.color * 0.25;
    return mix(ground_color, ambient.color, n.y * 0.5 + 0.5) * ambient.intensity;
}

// `ambient_radiance` averaged over every direction: what a particle, which has no
// normal, takes.
fn ambient_average(ambient: AmbientLight) -> vec3<f32> {
    if (ambient.mode > 1.5) {
        // Only the constant band survives the average.
        return max(ambient.sky_sh[0].xyz * 0.2820948, vec3<f32>(0.0));
    }
    if (ambient.mode > 0.5) {
        let ends = ambient.gradient_sky + ambient.gradient_ground;
        return (ambient.gradient_equator * 0.5 + ends * 0.25) * ambient.intensity;
    }
    return ambient.color * ambient.intensity * 0.625;
}

// `Skybox` mode: the sky's SH, already convolved and over pi, against the L2 basis
// (`lighting::sh::sh_basis`), clamped at zero as `Sh9::eval` is.
fn sky_sh(ambient: AmbientLight, n: vec3<f32>) -> vec3<f32> {
    let b = sh_basis(n);
    var out = vec3<f32>(0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        out += ambient.sky_sh[i].xyz * b[i];
    }
    return max(out, vec3<f32>(0.0));
}
