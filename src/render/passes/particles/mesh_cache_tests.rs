//! A mesh emitter's model follows the file (#1219): re-imported at a drawn spec it
//! draws the new mesh, and one that failed to load is picked up once it appears.

use super::{mark_stale, retry_appeared, CachedMesh};
use crate::components::{ParticleEmitterComponent, ParticleRender, ParticleRenderMode};
use crate::core::fs::freshness::MISS_RETRY_FRAMES;
use crate::core::fs::written::contract::{follows_rebakes, Bake};
use crate::core::fs::written::{note_written, written_since, Stale};
use crate::render::MeshId;
use std::collections::HashMap;

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

/// One spec naming `file`'s `Sub` object, cached as `cached`.
fn cache(file: &std::path::Path, cached: CachedMesh) -> (String, HashMap<String, CachedMesh>) {
    let spec = format!("{}::Sub", file.to_string_lossy());
    (spec.clone(), HashMap::from([(spec, cached)]))
}

#[test]
fn a_missing_file_that_appears_is_retried() {
    let file = crate::test_temp::dir().join("late.fbx");
    let (spec, mut meshes) = cache(&file, CachedMesh::Missing { existed: false });
    retry_appeared(&mut meshes);
    assert!(meshes.contains_key(&spec), "still absent: kept as a miss");
    std::fs::write(&file, "x").unwrap();
    retry_appeared(&mut meshes);
    assert!(!meshes.contains_key(&spec), "appeared: tried again");
}

#[test]
fn a_file_that_existed_and_failed_is_not_retried() {
    let file = crate::test_temp::dir().join("broken.fbx");
    std::fs::write(&file, "not an fbx").unwrap();
    let (spec, mut meshes) = cache(&file, CachedMesh::Missing { existed: true });
    retry_appeared(&mut meshes);
    assert!(
        meshes.contains_key(&spec),
        "a broken file is not re-imported on a timer"
    );
}

#[test]
fn a_note_on_the_source_marks_a_later_load_stale_but_not_its_own() {
    let file = crate::test_temp::dir().join("model.fbx");
    let path = file.to_string_lossy().into_owned();
    // The load's own import noted the source, then the load finished.
    note_written(&path);
    let at = written_since(u64::MAX).0;
    let id = MeshId("model".into());
    let (spec, mut meshes) = cache(&file, CachedMesh::Loaded { id, at });
    mark_stale(&mut meshes, &Stale::new(vec![path.clone()]));
    assert!(
        matches!(meshes[&spec], CachedMesh::Loaded { .. }),
        "its own note"
    );
    note_written(&path);
    mark_stale(&mut meshes, &Stale::new(vec![path.clone()]));
    assert_eq!(
        meshes[&spec],
        CachedMesh::Stale,
        "a re-import after the load"
    );
    let (spec, mut missing) = cache(&file, CachedMesh::Missing { existed: true });
    mark_stale(&mut missing, &Stale::new(vec![path]));
    assert_eq!(
        missing[&spec],
        CachedMesh::Stale,
        "a broken file re-imported"
    );
}
