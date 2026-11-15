//! crates/edit/src/authoring/particles/simulation.rs — the emitter's simulation
//! target (#2170): the op the card's *Simulation* dropdown and
//! `Particles.SetSimulation` both call, and what a `Gpu` emitter does not honour,
//! which every op that sets one of those settings warns about.

use rusty_components::particle::GPU_MAX_PARTICLES;
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
/// sub-emitters (they never fire), the `Mesh` render mode (it draws as a
/// billboard) and a `max_particles` above [`GPU_MAX_PARTICLES`] (it holds that
/// many). Empty for a `Cpu` emitter.
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
    if p.max_particles > GPU_MAX_PARTICLES {
        out.push(format!(
            "max_particles {} is above the GPU's cap: it holds {GPU_MAX_PARTICLES}",
            p.max_particles
        ));
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

    #[test]
    fn a_gpu_emitters_cap_is_clamped_with_a_warning_and_a_cpu_ones_is_not() {
        logs::init();
        let mut p = ParticleEmitterComponent::default();
        crate::authoring::particles::set_max_particles(&mut p, 3_000_000);
        assert_eq!(p.max_particles, 3_000_000);
        assert!(logs::take().is_empty(), "a Cpu emitter keeps any cap");
        set_simulation(&mut p, ParticleSimulation::Gpu);
        assert_eq!(logs::take().len(), 1, "the switch warns of the cap");
        assert_eq!(p.particle_cap(), GPU_MAX_PARTICLES, "and behaves as it");
        crate::authoring::particles::set_max_particles(&mut p, u32::MAX);
        assert_eq!(p.max_particles, GPU_MAX_PARTICLES);
        assert_eq!(logs::take().len(), 1, "one warning per set");
        crate::authoring::particles::set_max_particles(&mut p, 500_000);
        assert!(logs::take().is_empty());
    }

    #[test]
    fn a_change_of_space_forgets_a_gpu_emitters_particles_and_keeps_a_cpu_ones() {
        use crate::authoring::particles::set_simulation_space;
        use rusty_components::particle::EmitterFrame;
        use rusty_components::SimulationSpace;
        let mut cpu = ParticleEmitterComponent::default();
        cpu.emit_at(EmitterFrame::IDENTITY, 5);
        set_simulation_space(&mut cpu, SimulationSpace::Local);
        assert_eq!(cpu.live_count(), 5);
        let mut gpu = ParticleEmitterComponent::default();
        set_simulation(&mut gpu, ParticleSimulation::Gpu);
        gpu.emit_at(EmitterFrame::IDENTITY, 5);
        let epoch = gpu.runtime.gpu.epoch();
        set_simulation_space(&mut gpu, SimulationSpace::Local);
        assert_eq!(gpu.live_count(), 0, "the old space's particles are gone");
        assert_ne!(
            gpu.runtime.gpu.epoch(),
            epoch,
            "and the renderer's buffer with them"
        );
        set_simulation_space(&mut gpu, SimulationSpace::Local);
        gpu.emit_at(EmitterFrame::IDENTITY, 2);
        set_simulation_space(&mut gpu, SimulationSpace::Local);
        assert_eq!(gpu.live_count(), 2, "no change, nothing forgotten");
    }
}
