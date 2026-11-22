use std::rc::Rc;

use super::{EmitterRun, Sprites};
use rusty_components::particle::ParticleBlend;

fn run(texture: Option<&str>) -> EmitterRun {
    EmitterRun {
        blend: ParticleBlend::Alpha,
        texture: texture.map(|t| t.to_string().into()),
        sprites: Sprites::Instances(Vec::new()),
    }
}

#[test]
fn gpu_no_texture_binds_the_soft_disc_and_a_missing_file_the_checker() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let runs = vec![run(None), run(Some("no/such/sprite_2052.png")), run(None)];
    let (_, batches, _) = r.batch_runs(runs);
    assert_eq!(batches.len(), 3, "a different texture splits the draw");
    let sprite = &r.particle_renderer.default_sprite;
    assert!(
        Rc::ptr_eq(&batches[0].texture, sprite),
        "no texture: the disc"
    );
    assert!(Rc::ptr_eq(&batches[2].texture, sprite));
    assert!(
        Rc::ptr_eq(&batches[1].texture, &r.default_texture),
        "a failed load: the missing-texture checker"
    );
    assert!(!Rc::ptr_eq(sprite, &r.default_texture));
}
