//! Authored post-FX pixel tests (#397): a baked postfx module listed on the volume
//! reaches the image, a missing or broken module in the list is skipped (the frame
//! still renders, the rest of the list still runs), the FXAA-off path copies the
//! result out, and binding 3 hands an effect the previous frame's result.
//!
//! The effects are baked into the renderer's authored-shader dir under `t397_*`
//! names; each test writes only its own files. Built on the fog tests' flat,
//! unlit wall so a pixel is exactly the surface colour.

use rusty::components::CameraComponent;
use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;
use rusty::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};

use super::fog_scene::{centre, srgb8, wall_scene};

/// Bake a postfx module that zeroes red and blue, keeping green.
fn bake_green(name: &str) {
    let json = format!(
        r#"{{"pass":"postfx","name":"{name}","blocks":[{{"id":"tint","params":{{"color":[0,1,0]}}}}]}}"#
    );
    bake_recipe(
        &ShaderRecipe::from_json(&json).unwrap(),
        engine_shader_dir(),
        DEFAULT_OUT_DIR,
    )
    .expect("bake");
}

/// Write a hand-authored module straight into the authored-shader dir.
fn write_module(name: &str, source: &str) {
    std::fs::create_dir_all(DEFAULT_OUT_DIR).unwrap();
    std::fs::write(format!("{DEFAULT_OUT_DIR}/{name}.wgsl"), source).unwrap();
}

/// Remove this test's modules from the (gitignored) authored-shader dir.
fn cleanup(names: &[&str]) {
    for name in names {
        std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{name}.wgsl")).ok();
    }
}

/// Mix the colour so far 50/50 with binding 3, last frame's chain result.
const ECHO: &str = "@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var t_history: texture_2d<f32>;
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.uv = vec2<f32>(x, y);
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}
@fragment fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let now = textureSample(t_color, s_color, in.uv).rgb;
    return vec4<f32>(mix(now, textureSample(t_history, s_color, in.uv).rgb, 0.5), 1.0);
}";

/// A grey wall whose volume runs `effects`.
fn scene(effects: &[&str]) -> Scene {
    let mut scene = wall_scene(2.0, [0.5, 0.5, 0.5]);
    let id = scene.world.ids_with_visual_correction()[0];
    let mut vc = scene.world.visual_correction_mut(id).unwrap();
    vc.custom_effects = effects.iter().map(|s| s.to_string()).collect();
    drop(vc);
    scene
}

fn near(got: u8, want: i32) -> bool {
    (got as i32 - want).abs() <= 3
}

#[test]
fn listed_effects_reach_the_image_and_broken_ones_are_skipped() {
    bake_green("t397_green");
    // Composes, but has no `fs_main`: not a postfx module.
    write_module("t397_broken", "fn helper() -> f32 { return 1.0; }");
    let mut host = CaptureHost::new();
    let grey = srgb8(0.5);

    let Some(plain) = centre(&mut host, &scene(&[]), "pfx397_plain") else {
        return;
    };
    assert!(plain.iter().all(|&c| near(c, grey)), "plain wall {plain:?}");

    let list = ["t397_missing", "t397_broken", "t397_green"];
    let [r, g, b] = centre(&mut host, &scene(&list), "pfx397_green").unwrap();
    assert!(
        r <= 3 && b <= 3 && near(g, grey),
        "green-only {:?}",
        [r, g, b]
    );
}

#[test]
fn the_fxaa_off_path_still_lands_the_effects() {
    bake_green("t397_green_nofxaa");
    let mut scene = scene(&["t397_green_nofxaa"]);
    let id = scene.world.ids_with_visual_correction()[0];
    scene.world.set_camera(
        id,
        Some(CameraComponent {
            fov: 60.0,
            far: 100.0,
            fxaa_active: false,
            ..Default::default()
        }),
    );
    let Some([r, g, b]) = centre(&mut CaptureHost::new(), &scene, "pfx397_nofxaa") else {
        return;
    };
    assert!(r <= 3 && b <= 3 && near(g, srgb8(0.5)), "{:?}", [r, g, b]);
    cleanup(&["t397_green_nofxaa"]);
}

#[test]
fn binding_three_is_the_previous_frame() {
    bake_green("t397_green_echo");
    write_module("t397_echo", ECHO);
    let scene = scene(&["t397_green_echo", "t397_echo"]);
    let mut host = CaptureHost::new();
    // History starts black, so frame 1 is half the green, frame 2 three quarters.
    let Some(first) = centre(&mut host, &scene, "pfx397_echo1") else {
        return;
    };
    let second = centre(&mut host, &scene, "pfx397_echo2").unwrap();
    assert!(near(first[1], srgb8(0.25)), "frame 1 {first:?}");
    assert!(near(second[1], srgb8(0.375)), "frame 2 {second:?}");
    cleanup(&["t397_green_echo", "t397_echo"]);
}
