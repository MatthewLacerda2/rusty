//! crates/components/src/particle/simulation/ — the per-emitter **simulation
//! target** (Niagara's *CPUSim* / *GPUCompute Sim*, #2170).
//!
//! A `Cpu` emitter keeps its live particles in `EmitterRuntime::particles` and the
//! sim moves them (`app::systems::particles`). A `Gpu` emitter's live particles are
//! **render state**: the renderer keeps them in a GPU buffer, moves them in a
//! compute pass and draws them from it, so the sim never reads them. What the sim
//! keeps of a `Gpu` emitter is [`GpuSim`]:
//! - its **births**, built on the CPU through the same seeded spawn path a `Cpu`
//!   emitter uses (every shape, range and draw order), each given a slot of the
//!   GPU buffer, for the renderer to upload;
//! - an exact **live count** with no readback: each particle's lifetime is known at
//!   birth, so the particles born before the same step are one *cohort* that ages
//!   together by the same `dt`s, exactly as the CPU path ages each particle
//!   (`age += dt`, dead once `age >= lifetime`), and a cohort lets its particles
//!   go in lifetime order. `Particles.GetCount` and the `max_particles` cap read it;
//! - the **step** count, every step's `dt` (replayed one by one by the renderer,
//!   so one adapter draws the same particles whatever the frame rate) and the
//!   running **time** (`history`), which ages a jump past the replayed window.
//!
//! Nothing here reads what the GPU computed, which is what keeps the sim a pure
//! function of (seed, inputs, dt).

mod history;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use super::runtime::Particle;
use history::History;

rusty_core::named_enum! {
    /// Where an emitter's live particles are simulated.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ParticleSimulation {
        /// On the CPU, by the sim: they collide, fire death and collision
        /// sub-emitters, and scripts read them (gameplay particles).
        #[default]
        Cpu = "Cpu",
        /// On the GPU, by the renderer: visual only, for dense effects (a room of
        /// smoke, a debris cloud, rain); births stay on the CPU.
        Gpu = "Gpu",
    }
}

/// How many steps' `dt`s are kept for the renderer to replay, one by one, in one
/// dispatch: it bounds the compute step's loop. A renderer that draws less often
/// than this (a long `Harness.Step`) moves a particle over the older steps in one
/// closed-form jump, aged by the time they took ([`GpuSim::time_at`],
/// [`Birth::time`]) and moved at their mean `dt`.
pub const DT_HISTORY: usize = 256;

/// The most particles a `Gpu` emitter holds, whatever its `max_particles`
/// (`ParticleEmitterComponent::particle_cap`): fixed, so the sim's cap, and
/// `Particles.GetCount`, are the same on every device. 64 MB of particle buffer.
pub const GPU_MAX_PARTICLES: u32 = 1_000_000;

/// One particle born on the CPU for the GPU, waiting to be uploaded.
#[derive(Clone, Copy, Debug)]
pub struct Birth {
    /// The particle at birth: in world space, or for a `Local` emitter in the
    /// space its draw places it under (`ParticleEmitterComponent::gpu_space`).
    pub particle: Particle,
    /// Its slot in the emitter's GPU buffer.
    pub slot: u32,
    /// The step count when it was born: its first step is step `step + 1`.
    pub step: u64,
    /// The running time when it was born ([`GpuSim::time`] then).
    pub time: f64,
    /// Its birth number on this emitter, counting from 0 at the last reset.
    pub serial: u64,
    /// Whether it has died since (kept until a compaction drops it): the renderer
    /// uploads none such.
    pub dead: bool,
}

/// One cohort: the particles born before the same step, aging together.
#[derive(Clone, Debug, Default)]
struct Cohort {
    age: f32,
    /// `(lifetime, slot, serial)`, longest lifetime first, so the next to die is
    /// at the back.
    members: Vec<(f32, u32, u64)>,
}

/// What the sim keeps of a `Gpu` emitter (see the module docs). Reset on entering
/// Play and by `Particles.Clear`.
#[derive(Clone, Debug)]
pub struct GpuSim {
    /// The steps taken since the last reset, and the last ones' `dt`s and times.
    history: History,
    /// Particles live now.
    live: usize,
    /// Born since the last step: they age from the next one.
    newborn: Cohort,
    cohorts: VecDeque<Cohort>,
    /// Slots freed by a death, reused before a fresh one.
    free: Vec<u32>,
    /// One past the highest slot ever handed out: the buffer's live extent.
    extent: u32,
    /// Births not yet known dead, oldest first (dead ones may sit behind a live
    /// one until a compaction).
    births: VecDeque<Birth>,
    /// The next birth's serial.
    serial: u64,
    /// Births known dead since the last compaction.
    dead: usize,
    /// Which reset this state is: a fresh number on every reset, from a process
    /// counter, so the renderer knows to forget its buffer. The sim never reads it.
    epoch: u64,
    /// How many times every particle has died and the extent gone back to 0: the
    /// renderer empties its buffer when it changed, so a slot a birth it never saw
    /// was handed is not left holding an older particle.
    emptied: u64,
    /// Whether a script's write of the live particles was warned about yet.
    warned: bool,
}

/// Source of [`GpuSim::epoch`]s. Render bookkeeping only: no sim value depends on it.
static EPOCHS: AtomicU64 = AtomicU64::new(1);

impl Default for GpuSim {
    fn default() -> Self {
        Self {
            history: History::default(),
            live: 0,
            newborn: Cohort::default(),
            cohorts: VecDeque::new(),
            free: Vec::new(),
            extent: 0,
            births: VecDeque::new(),
            serial: 0,
            dead: 0,
            epoch: EPOCHS.fetch_add(1, Ordering::Relaxed),
            emptied: 0,
            warned: false,
        }
    }
}

impl GpuSim {
    /// Particles live now, exactly as many as a `Cpu` emitter of the same config
    /// and seed would hold (collision `None`).
    pub fn live(&self) -> usize {
        self.live
    }

    /// Steps taken since the last reset.
    pub fn steps(&self) -> u64 {
        self.history.steps
    }

    /// One past the highest slot handed out: how much of the buffer to draw.
    pub fn extent(&self) -> u32 {
        self.extent
    }

    /// Which reset this state is (see the field).
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// How many times the extent has gone back to 0 (see the field).
    pub fn emptied(&self) -> u64 {
        self.emptied
    }

    /// The next birth's serial: every birth below it has been made.
    pub fn next_serial(&self) -> u64 {
        self.serial
    }

    /// The births kept, oldest first (each one at most once per serial).
    pub fn births(&self) -> impl Iterator<Item = &Birth> {
        self.births.iter()
    }

    /// The births kept whose serial is `from` or later, oldest first: found by a
    /// binary search, so a caller reading only what is new pays for what is new.
    pub fn births_from(&self, from: u64) -> impl Iterator<Item = &Birth> {
        let at = self.births.partition_point(|b| b.serial < from);
        self.births.range(at..)
    }

    /// The `dt` step `step` (1-based) took, or the oldest kept one when it is older
    /// than the history; `None` before any step.
    pub fn dt_of(&self, step: u64) -> Option<f32> {
        self.history.dt_of(step)
    }

    /// The running time now: the `f64` sum of every step's `dt` since the last
    /// reset, added in step order, so it is the same on every run.
    pub fn time(&self) -> f64 {
        self.history.time()
    }

    /// The running time after step `step` (0 is the reset), or `None` when it is
    /// older than the kept steps (more than [`DT_HISTORY`] ago) or not taken yet.
    pub fn time_at(&self, step: u64) -> Option<f64> {
        self.history.time_at(step)
    }

    /// `true` the first time it is asked since the last reset: what a write a
    /// `Gpu` emitter ignores (`Particles.SetParticles`) warns by, once.
    pub fn first_warning(&mut self) -> bool {
        !std::mem::replace(&mut self.warned, true)
    }

    /// Forget every particle, as `Particles.Clear` does: the renderer drops its
    /// buffer on the new epoch.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Add one particle born now, in a free slot. The caller checks the cap.
    pub fn add(&mut self, particle: Particle) {
        let slot = self.free.pop().unwrap_or_else(|| {
            self.extent += 1;
            self.extent - 1
        });
        let serial = self.serial;
        self.serial += 1;
        self.newborn.members.push((particle.lifetime, slot, serial));
        self.births.push_back(Birth {
            particle,
            slot,
            step: self.history.steps,
            time: self.time(),
            serial,
            dead: false,
        });
        self.live += 1;
    }

    /// Take one step of `dt`: every particle ages by `dt`, and each whose age
    /// reaches its lifetime (or whose lifetime is not a number) dies, freeing its
    /// slot. The CPU path's `age += dt; alive = age < lifetime`, cohort by cohort.
    pub fn step(&mut self, dt: f32) {
        self.history.push(dt);
        if !self.newborn.members.is_empty() {
            let mut cohort = std::mem::take(&mut self.newborn);
            cohort
                .members
                .sort_by(|a, b| b.0.total_cmp(&a.0).then(a.2.cmp(&b.2)));
            // NaN sorts above every number: it dies on its first step, like the
            // CPU path's `age < NaN`, so it goes at the back.
            let nan = cohort.members.iter().take_while(|m| m.0.is_nan()).count();
            cohort.members.rotate_left(nan);
            self.cohorts.push_back(cohort);
        }
        let mut deaths = Vec::new();
        for cohort in &mut self.cohorts {
            cohort.age += dt;
            while let Some(&(lifetime, slot, serial)) = cohort.members.last() {
                if cohort.age < lifetime {
                    break;
                }
                cohort.members.pop();
                deaths.push((slot, serial));
            }
        }
        self.cohorts.retain(|c| !c.members.is_empty());
        for (slot, serial) in deaths {
            self.free.push(slot);
            self.live -= 1;
            self.forget(serial);
        }
        if self.live == 0 && self.extent > 0 {
            // Nothing alive: slots start again from 0, and the buffer's extent with them.
            self.free.clear();
            self.extent = 0;
            self.emptied += 1;
        }
    }

    /// Mark the birth numbered `serial` dead: pop the dead off the front, and
    /// compact once the dead behind a live one outnumber the live.
    fn forget(&mut self, serial: u64) {
        if let Ok(at) = self.births.binary_search_by_key(&serial, |b| b.serial) {
            if at == 0 {
                self.births.pop_front();
            } else {
                self.births[at].dead = true;
                self.dead += 1;
            }
        }
        while self.births.front().is_some_and(|b| b.dead) {
            self.births.pop_front();
            self.dead -= 1;
        }
        if self.dead > self.live + 1024 {
            self.births.retain(|b| !b.dead);
            self.dead = 0;
        }
    }
}

#[cfg(test)]
mod tests;
