//! crates/edit/src/authoring/particles/simulation.rs — the emitter's simulation
//! target (#2170): the op the card's *Simulation* dropdown and
//! `Particles.SetSimulation` both call, and what a `Gpu` emitter does not honour,
//! which every op that sets one of those settings warns about.

use rusty_components::{
    CollisionResponse, ParticleEmitterComponent, ParticleRenderMode, ParticleSimulation,
};

/// Set the simulation target (#2170). A change forgets the live particles (the
/// other target never held them), and a switch to `Gpu` warns for each setting
/// a `Gpu` emitter does not honour ([`gpu_warnings`]).
pub fn set_simulation(p: &mut ParticleEmitterComponent, simulation: ParticleSimulation) {
    if p.simulation != simulation {
        p.clear_particles();
        p.runtime.anchor = None;
        p.simulation = simulation;
        warn_gpu(p);
    }
}

/// What a `Gpu` emitter does not honour of `p`'s settings, one line each: a
/// collision response (it behaves as `None`), the death and collision
/// sub-emitters (they never fire) and the `Mesh` render mode (it draws as a
/// billboard). Empty for a `Cpu` emitter.
pub fn gpu_warnings(p: &ParticleEmitterComponent) -> Vec<String> {
    if !p.on_gpu() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if p.collision != CollisionResponse::None {
        out.push(format!(
            "collision {:?} is not simulated on the GPU: it behaves as None",
            p.collision
        ));
    }
    for (trigger, target) in [
        ("death", p.sub_emitters.death),
        ("collision", p.sub_emitters.collision),
    ] {
        if target.is_some() {
            out.push(format!("the {trigger} sub-emitter never fires on the GPU"));
        }
    }
    if p.render.mode == ParticleRenderMode::Mesh {
        out.push("mesh particles are not drawn on the GPU: they draw as billboards".into());
    }
    out
}

/// Log [`gpu_warnings`].
pub(super) fn warn_gpu(p: &ParticleEmitterComponent) {
    for line in gpu_warnings(p) {
        log::warn!("[Particles] Gpu emitter: {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authoring::particles::{set_collision, set_render_mode};
    use rusty_test_support::logs;

    #[test]
    fn a_switch_to_gpu_warns_for_each_setting_it_does_not_honour() {
        logs::init();
        let mut p = ParticleEmitterComponent {
            collision: CollisionResponse::Bounce,
            ..Default::default()
        };
        p.sub_emitters.death = Some(4);
        p.render.mode = ParticleRenderMode::Mesh;
        p.runtime.particles.push(rusty_components::Particle::at(
            glam::Vec3::ZERO,
            1.0,
            [1.0; 4],
            1.0,
        ));
        assert!(
            gpu_warnings(&p).is_empty(),
            "a Cpu emitter honours them all"
        );
        set_simulation(&mut p, ParticleSimulation::Gpu);
        assert!(
            p.runtime.particles.is_empty(),
            "the switch forgets the live ones"
        );
        let said = logs::take();
        assert_eq!(said.len(), 3, "{said:?}");
        assert!(said.iter().all(|s| s.contains("Gpu emitter")));
        set_simulation(&mut p, ParticleSimulation::Gpu);
        assert!(logs::take().is_empty(), "no change, no warning");
    }

    #[test]
    fn setting_a_dishonoured_setting_on_a_gpu_emitter_warns_and_on_cpu_does_not() {
        logs::init();
        let mut p = ParticleEmitterComponent::default();
        set_collision(&mut p, CollisionResponse::Die);
        assert!(logs::take().is_empty());
        set_collision(&mut p, CollisionResponse::None);
        set_simulation(&mut p, ParticleSimulation::Gpu);
        set_collision(&mut p, CollisionResponse::Die);
        set_render_mode(&mut p, ParticleRenderMode::Mesh);
        assert_eq!(logs::take().len(), 3, "collision, then collision and mesh");
    }
}
