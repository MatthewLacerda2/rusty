//! A renderer that misses more steps than the GPU replays one by one: the older
//! ones are one jump, aged by the time they really took. Here the replayed window
//! is all paused (time scale 0) while the steps before it moved, so a jump aged by
//! the window's `dt` would leave every particle young: drawn where it was born and
//! alive past the sim's tally.

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::particle::{EmitMode, EmitShape, ParticleSimulation};
use rusty::core::curve::Range;
use rusty::dev::capture::CaptureHost;

use super::{agreement, emitter, red, world};

fn count(game: &GameWorld, id: u32) -> usize {
    game.scene()
        .borrow()
        .world
        .particles(id)
        .unwrap()
        .live_count()
}

#[test]
fn a_jump_over_a_pause_ages_by_the_time_that_passed() {
    let mut host = CaptureHost::new();
    let config = |simulation| {
        let mut e = emitter(simulation);
        e.shape = EmitShape::Sphere { radius: 0.5 };
        e.direction = Vec3::X;
        e.speed = Range::new(0.3, 0.8);
        e.size = Range::new(0.15, 0.3);
        e.lifetime = Range::new(3.0, 6.0);
        e.emit_mode = EmitMode::Continuous;
        e.rate = 40.0;
        e.seed = 2170;
        e
    };
    let (mut gpu, gpu_id) = world(config(ParticleSimulation::Gpu));
    let (mut cpu, cpu_id) = world(config(ParticleSimulation::Cpu));
    // 300 ticks moving, then 300 paused, with no render in between: more than
    // the 256 steps the GPU replays, all of them paused.
    for game in [&mut gpu, &mut cpu] {
        crate::play::run(game, 300);
        game.time().borrow_mut().time_scale = 0.0;
        crate::play::run(game, 300);
    }
    // One frame after the pause, then time runs again and the tally kills the
    // particles born first: each frame draws what its `Cpu` twin draws.
    for (frame, ticks) in [0, 60, 60, 60].into_iter().enumerate() {
        for game in [&mut gpu, &mut cpu] {
            game.time().borrow_mut().time_scale = if ticks == 0 { 0.0 } else { 1.0 };
            crate::play::run(game, ticks);
        }
        assert_eq!(count(&gpu, gpu_id), count(&cpu, cpu_id), "frame {frame}");
        let (Some(a), Some(b)) = (
            red(
                &mut host,
                &gpu.scene().borrow(),
                &format!("pause_gpu_{frame}"),
            ),
            red(
                &mut host,
                &cpu.scene().borrow(),
                &format!("pause_cpu_{frame}"),
            ),
        ) else {
            return;
        };
        assert!(
            a.iter().filter(|&&x| x).count() > 50,
            "frame {frame}: drawn"
        );
        // Measured on lavapipe (Mesa, the container): see the PR's Gates line.
        let agree = agreement(&a, &b);
        assert!(agree > 0.98, "frame {frame}: {agree}");
    }
}
