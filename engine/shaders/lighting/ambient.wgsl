// engine/shaders/lighting/ambient.wgsl — the scene's ambient light as the shaders
// read it (#1647): the `Color` hemisphere or the `Gradient` trilight, the WGSL side of
// `scene::settings::ambient`. The data is `common::AmbientLight` (in
// `common::LightingUniforms`); the forward pass (`forward::lighting::flat_ambient`)
// and lit particles evaluate it here.

#define_import_path lighting::ambient

#import common::AmbientLight

// The ambient a surface with unit normal `n` takes, intensity applied, as
// irradiance / pi. `Color` mode is the hemisphere the forward pass has always
// drawn, `color` straight up fading to a quarter of it straight down; `Gradient` is Unity's trilight, linear in `n.y` from the ground
// through the equator to the sky. The CPU's `scene::AmbientRadiance::at` is the same
// function (a GPU test holds the two equal); lightmaps and probes bake its
// `incoming` radiance, whose cosine average over a hemisphere is this again.
fn ambient_radiance(ambient: AmbientLight, n: vec3<f32>) -> vec3<f32> {
    if (ambient.gradient > 0.5) {
        let toward = select(ambient.gradient_ground, ambient.gradient_sky, n.y >= 0.0);
        return mix(ambient.gradient_equator, toward, abs(n.y)) * ambient.intensity;
    }
    let ground_color = ambient.color * 0.25;
    return mix(ground_color, ambient.color, n.y * 0.5 + 0.5) * ambient.intensity;
}

// `ambient_radiance` averaged over every direction: what a particle, which has no
// normal, takes.
fn ambient_average(ambient: AmbientLight) -> vec3<f32> {
    if (ambient.gradient > 0.5) {
        let ends = ambient.gradient_sky + ambient.gradient_ground;
        return (ambient.gradient_equator * 0.5 + ends * 0.25) * ambient.intensity;
    }
    return ambient.color * ambient.intensity * 0.625;
}
