use rusty_components::particle::GPU_MAX_PARTICLES;
use rusty_components::{
    CollisionResponse, ParticleEmitterComponent, ParticleRenderMode, ParticleSimulation,
    SimulationSpace, SubEmitTrigger,
};

pub fn set_simulation(p: &mut ParticleEmitterComponent, simulation: ParticleSimulation) {
    let was = p.simulation;
    switch_simulation(p, simulation);
    warn_switch(was, p);
}

/// `set_simulation` without its warnings, for `Component.Set`'s draft: [`commit`] warns once, for
/// the emitter as written.
pub fn switch_simulation(p: &mut ParticleEmitterComponent, simulation: ParticleSimulation) {
    retarget(p, simulation, p.simulation_space);
}

fn warn_switch(was: ParticleSimulation, p: &ParticleEmitterComponent) {
    if was != p.simulation {
        for line in gpu_warnings(p) {
            log_gpu(&line);
        }
    }
}

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
    // Forget the frames the particles were carried by, so none moves by a stale matrix.
    p.runtime.anchor = None;
    p.runtime.base = None;
    p.simulation = simulation;
    p.simulation_space = space;
}

pub fn draft(live: &ParticleEmitterComponent) -> ParticleEmitterComponent {
    live.config()
}

pub fn commit(live: &mut ParticleEmitterComponent, mut draft: ParticleEmitterComponent) {
    let (simulation, space) = (draft.simulation, draft.simulation_space);
    draft.simulation = live.simulation;
    draft.simulation_space = live.simulation_space;
    draft.runtime = std::mem::take(&mut live.runtime);
    let was = live.simulation;
    *live = draft;
    retarget(live, simulation, space);
    warn_switch(was, live);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Dishonoured {
    Collision,
    /// The sub-emitter fired at this trigger (death or collision): it never fires.
    SubEmitter(SubEmitTrigger),
    Mesh,
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

    pub(super) fn warn(self, p: &ParticleEmitterComponent) {
        if let Some(line) = self.line(p) {
            log_gpu(&line);
        }
    }
}

pub fn gpu_warnings(p: &ParticleEmitterComponent) -> Vec<String> {
    Dishonoured::ALL.iter().filter_map(|d| d.line(p)).collect()
}

fn log_gpu(line: &str) {
    log::warn!("[Particles] Gpu emitter: {line}");
}

#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
