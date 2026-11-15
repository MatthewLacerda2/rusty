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
