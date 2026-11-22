mod history;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use super::runtime::Particle;
use history::History;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ParticleSimulation {
        #[default]
        Cpu = "Cpu",
        Gpu = "Gpu",
    }
}

pub const DT_HISTORY: usize = 256;

pub const GPU_MAX_PARTICLES: u32 = 1_000_000;

#[derive(Clone, Copy, Debug)]
pub struct Birth {
    /// The particle at birth: in world space, or for a `Local` emitter in the
    /// space its draw places it under (`ParticleEmitterComponent::gpu_space`).
    pub particle: Particle,
    pub slot: u32,
    pub step: u64,
    pub time: f64,
    pub serial: u64,
    pub dead: bool,
}

#[derive(Clone, Debug, Default)]
struct Cohort {
    age: f32,
    members: Vec<(f32, u32, u64)>,
}

#[derive(Clone, Debug)]
pub struct GpuSim {
    history: History,
    live: usize,
    newborn: Cohort,
    cohorts: VecDeque<Cohort>,
    free: Vec<u32>,
    extent: u32,
    /// Births not yet known dead, oldest first (dead ones may sit behind a live
    /// one until a compaction).
    births: VecDeque<Birth>,
    serial: u64,
    dead: usize,
    epoch: u64,
    emptied: u64,
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
    pub fn live(&self) -> usize {
        self.live
    }

    pub fn steps(&self) -> u64 {
        self.history.steps
    }

    pub fn extent(&self) -> u32 {
        self.extent
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn emptied(&self) -> u64 {
        self.emptied
    }

    pub fn next_serial(&self) -> u64 {
        self.serial
    }

    /// The births kept, oldest first (each one at most once per serial).
    pub fn births(&self) -> impl Iterator<Item = &Birth> {
        self.births.iter()
    }

    pub fn births_from(&self, from: u64) -> impl Iterator<Item = &Birth> {
        let at = self.births.partition_point(|b| b.serial < from);
        self.births.range(at..)
    }

    pub fn dt_of(&self, step: u64) -> Option<f32> {
        self.history.dt_of(step)
    }

    pub fn time(&self) -> f64 {
        self.history.time()
    }

    pub fn time_at(&self, step: u64) -> Option<f64> {
        self.history.time_at(step)
    }

    pub fn first_warning(&mut self) -> bool {
        !std::mem::replace(&mut self.warned, true)
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

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

    pub fn step(&mut self, dt: f32) {
        self.history.push(dt);
        if !self.newborn.members.is_empty() {
            let mut cohort = std::mem::take(&mut self.newborn);
            cohort
                .members
                .sort_by(|a, b| b.0.total_cmp(&a.0).then(a.2.cmp(&b.2)));
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
            self.free.clear();
            self.extent = 0;
            self.emptied += 1;
        }
    }

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
