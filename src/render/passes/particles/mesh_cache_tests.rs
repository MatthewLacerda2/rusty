//! A mesh emitter's model follows the file (#1219): re-imported at a drawn spec it
//! draws the new mesh, and one that failed to load is picked up once it appears.

use crate::components::{ParticleEmitterComponent, ParticleRender, ParticleRenderMode};
use crate::core::fs::written::contract::{follows_rebakes, Bake};
use crate::render::gpu::textures::freshness::MISS_RETRY_FRAMES;

/// A two-triangle quad (6 indices).
const QUAD: &str = "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3\nf 1 3 4\n";
/// One triangle (3 indices), under the same object name.
const TRIANGLE: &str = "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nf 1 2 3\n";

fn emitter(spec: &str) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        render: ParticleRender {
            mode: ParticleRenderMode::Mesh,
            mesh: Some(spec.to_string()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn indices(bake: Bake) -> u32 {
    if bake == Bake::A {
        6
    } else {
        3
    }
}

/// The shared cache contract (#1207): drawn before the model exists (a miss),
/// imported (A), re-imported (B), each draw shows what is on disk.
#[test]
fn gpu_a_reimported_model_reaches_its_mesh_emitter() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let path = crate::test_temp::dir().join("casing.obj");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, bake| {
            let text = if bake == Bake::A { QUAD } else { TRIANGLE };
            crate::core::fs::written::write(std::path::Path::new(p), text.as_bytes()).unwrap();
        },
        |p| {
            renderer.refresh_textures();
            let spec = format!("{p}::Model");
            renderer.particle_mesh(&emitter(&spec)).map(|(_, n)| n)
        },
        |loaded, bake| *loaded == Some(indices(bake)),
    );
}

/// A model copied in by an outside tool (no note) after the first draw is picked
/// up on the next retry, not cached as missing for good.
#[test]
fn gpu_a_model_that_appears_later_is_retried() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let path = crate::test_temp::dir().join("late.obj");
    let spec = format!("{}::Model", path.to_string_lossy());
    let emitter = emitter(&spec);
    assert!(renderer.particle_mesh(&emitter).is_none());
    std::fs::write(&path, QUAD).unwrap();
    renderer.refresh_textures();
    assert!(
        renderer.particle_mesh(&emitter).is_none(),
        "a miss is throttled, not re-read every frame"
    );
    for _ in 0..MISS_RETRY_FRAMES {
        renderer.refresh_textures();
    }
    let loaded = renderer.particle_mesh(&emitter).map(|(_, n)| n);
    assert_eq!(loaded, Some(6));
}
