#define_import_path depth

// Which way a view's depth runs (`rusty_scene::camera::depth`, `render::gpu::depth`): reversed,
// the near plane at 1 and the far plane at 0, so the nearer surface has the greater depth. A
// shader that reads or writes view depth goes through these, never a 0 or a 1 of its own; shadow
// maps keep their own ordinary depth. A test holds the constants equal to the Rust ones.

const NEAR: f32 = 1.0;
const FAR: f32 = 0.0;

// A texel no surface drew: the clear, where the sky shows.
fn is_sky(d: f32) -> bool {
    return d <= FAR;
}

// Whether depth `a` lies in front of depth `b`, nearer the eye.
fn in_front(a: f32, b: f32) -> bool {
    return a > b;
}

// The nearer and the farther of two depths.
fn nearer(a: f32, b: f32) -> f32 {
    return max(a, b);
}

fn farther(a: f32, b: f32) -> f32 {
    return min(a, b);
}

// How far depth `a` lies behind depth `b`, in depth units: positive when `b` is the nearer.
fn behind(a: f32, b: f32) -> f32 {
    return (a - b) * (FAR - NEAR);
}

// A clip-space point between the eye and the near plane (or behind it, with `w` above zero).
fn before_near(clip: vec4<f32>) -> bool {
    return clip.z > clip.w;
}

// A clip-space position moved onto the far plane, where only pixels no surface drew pass.
fn on_far_plane(clip: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(clip.xy, clip.w * FAR, clip.w);
}
