//! crates/edit/src/authoring/particles/simulation.rs — the emitter's simulation
//! target (#2170): the op the card's *Simulation* dropdown and
//! `Particles.SetSimulation` both call, and what a `Gpu` emitter does not honour,
//! which every op that sets one of those settings warns about.

use rusty_components::particle::GPU_MAX_PARTICLES;
use rusty_components::{
    CollisionResponse, ParticleEmitterComponent, ParticleRenderMode, ParticleSimulation,
    SimulationSpace, SubEmitTrigger,
};

/// Set the simulation target (#2170). A change forgets the live particles (the
/// other target never held them), and a switch to `Gpu` warns for each setting
/// a `Gpu` emitter does not honour ([`gpu_warnings`]).
pub fn set_simulation(p: &mut ParticleEmitterComponent, simulation: ParticleSimulation) {
    if p.simulation != simulation {
        retarget(p, simulation, p.simulation_space);
        for line in gpu_warnings(p) {
            log_gpu(&line);
        }
    }
}

/// Move `p` to `simulation` in `space`, forgetting what of its live state the
/// move makes stale: any change forgets where the live particles were last placed
/// (`anchor`), so nothing is carried by a stale matrix; a change of target forgets
/// the live particles (the other target never held them), and so does a change of
/// space on a `Gpu` emitter, whose particles are stored in the old space on both
/// sides with nothing to carry them. The one rule `set_simulation`,
/// `set_simulation_space` and `Component.Set`'s commit ([`commit`]) share.
pub(super) fn retarget(
    p: &mut ParticleEmitterComponent,
    simulation: ParticleSimulation,
    space: SimulationSpace,
) {
    let target = p.simulation != simulation;
    let moved = p.simulation_space != space;
    if !(target || moved) {
        return;
    }
    if target || simulation == ParticleSimulation::Gpu {
        p.clear_particles();
    }
    p.runtime.anchor = None;
    p.simulation = simulation;
    p.simulation_space = space;
}

/// `Component.Set`'s copy of an emitter to run its ops on: its configuration,
/// without the live state, so an edit never copies the live particles (a `Gpu`
/// emitter's births, up to one per live particle).
pub fn draft(live: &ParticleEmitterComponent) -> ParticleEmitterComponent {
    live.config()
}

/// Put `Component.Set`'s finished [`draft`] in place of `live`, keeping `live`'s
/// live state but for what the new target and space make stale (`retarget`).
pub fn commit(live: &mut ParticleEmitterComponent, mut draft: ParticleEmitterComponent) {
    let (simulation, space) = (draft.simulation, draft.simulation_space);
    draft.simulation = live.simulation;
    draft.simulation_space = live.simulation_space;
    draft.runtime = std::mem::take(&mut live.runtime);
    *live = draft;
    retarget(live, simulation, space);
}

/// A setting a `Gpu` emitter does not honour.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Dishonoured {
    /// A collision response: it behaves as `None`.
    Collision,
    /// The sub-emitter fired at this trigger (death or collision): it never fires.
    SubEmitter(SubEmitTrigger),
    /// The `Mesh` render mode: it draws as a billboard.
    Mesh,
    /// A `max_particles` above [`GPU_MAX_PARTICLES`]: it holds that many.
    Cap,
}

impl Dishonoured {
    const ALL: [Self; 5] = [
        Self::Collision,
        Self::SubEmitter(SubEmitTrigger::Death),
        Self::SubEmitter(SubEmitTrigger::Collision),
        Self::Mesh,
        Self::Cap,
    ];

    /// What `p` does not honour of this setting, or `None` when it honours it
    /// (always, on a `Cpu` emitter).
    fn line(self, p: &ParticleEmitterComponent) -> Option<String> {
        if !p.on_gpu() {
            return None;
        }
        match self {
            Self::Collision => (p.collision != CollisionResponse::None).then(|| {
                format!(
                    "collision {:?} is not simulated on the GPU: it behaves as None",
                    p.collision
                )
            }),
            // A birth sub-emitter fires: births happen on the CPU.
            Self::SubEmitter(SubEmitTrigger::Birth) => None,
            Self::SubEmitter(trigger) => p
                .sub_emitters
                .get(trigger)
                .map(|_| format!("the {} sub-emitter never fires on the GPU", trigger.name())),
            Self::Mesh => (p.render.mode == ParticleRenderMode::Mesh)
                .then(|| "mesh particles are not drawn on the GPU: they draw as billboards".into()),
            Self::Cap => (p.max_particles > GPU_MAX_PARTICLES).then(|| {
                format!(
                    "max_particles {} is above the GPU's cap: it holds {GPU_MAX_PARTICLES} \
                     (the stored value is kept, and holds again on Cpu)",
                    p.max_particles
                )
            }),
        }
    }

    /// Log this setting's line, if `p` does not honour it: what an op that just
    /// set it says, and nothing about the others.
    pub(super) fn warn(self, p: &ParticleEmitterComponent) {
        if let Some(line) = self.line(p) {
            log_gpu(&line);
        }
    }
}

/// What a `Gpu` emitter does not honour of `p`'s settings, one line each
/// (`Dishonoured`): a collision response, the death and collision
/// sub-emitters, the `Mesh` render mode and a `max_particles` above
/// [`GPU_MAX_PARTICLES`]. Empty for a `Cpu` emitter.
pub fn gpu_warnings(p: &ParticleEmitterComponent) -> Vec<String> {
    Dishonoured::ALL.iter().filter_map(|d| d.line(p)).collect()
}

fn log_gpu(line: &str) {
    log::warn!("[Particles] Gpu emitter: {line}");
}

#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
