//! crates/script/src/api/particle/gpu.rs — the live particle calls on a `Gpu`
//! emitter (#2170), whose particles are the renderer's: every read
//! (`GetParticles`, `GetArrays`, `ForEach`) sees none, so `ForEach` writes back
//! nothing, and a write (`SetParticles`, `SetArrays`) changes nothing and warns,
//! once between them ([`refuse`]).

use rusty_components::particle::ParticleEmitterComponent;

/// A write (`verb`) to entity `id`'s `Gpu` emitter `e`: nothing changes, the
/// first one warns, and the number live is returned.
pub(super) fn refuse(e: &mut ParticleEmitterComponent, verb: &str, id: u32) -> u32 {
    if e.runtime.gpu.first_warning() {
        log::warn!(
            "[Particles] {verb}: entity {id} simulates on the GPU; its particles cannot \
             be written (the call does nothing)"
        );
    }
    e.live_count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_components::particle::EmitterFrame;
    use rusty_components::ParticleSimulation;
    use rusty_test_support::logs;

    #[test]
    fn a_write_to_a_gpu_emitter_warns_once_and_changes_nothing() {
        logs::init();
        let mut e = ParticleEmitterComponent {
            simulation: ParticleSimulation::Gpu,
            ..Default::default()
        };
        e.emit_at(EmitterFrame::IDENTITY, 3);
        assert_eq!(refuse(&mut e, "SetParticles", 1), 3);
        assert_eq!(logs::take().len(), 1);
        assert_eq!(refuse(&mut e, "SetArrays", 1), 3);
        assert!(logs::take().is_empty(), "once");
        assert_eq!(e.live_count(), 3);
    }
}
