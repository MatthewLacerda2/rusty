mod run;
pub(crate) mod tween;
pub(crate) mod wait;

use std::collections::BTreeMap;

use mlua::RegistryKey;

pub(crate) use wait::Wait;

const ELAPSED_EPSILON: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clock {
    Scaled,
    Unscaled,
}

pub(crate) enum Target {
    Func(RegistryKey),
    Name(String),
}

pub(crate) enum Work {
    Invoke {
        target: Target,
        interval: Option<f64>,
    },
    Coroutine {
        thread: RegistryKey,
    },
    Tween(Box<tween::Tween>),
}

pub(crate) struct Job {
    pub(crate) owner: u32,
    pub(crate) work: Work,
    pub(crate) wait: Wait,
    elapsed: f64,
    born: u64,
}

#[derive(Default)]
pub struct TimerScheduler {
    jobs: BTreeMap<u64, Job>,
    next_handle: u64,
    epoch: u64,
}

impl TimerScheduler {
    pub(crate) fn alloc(&mut self) -> u64 {
        self.next_handle += 1;
        self.next_handle
    }

    pub(crate) fn insert(&mut self, handle: u64, owner: u32, work: Work, wait: Wait) {
        let born = self.epoch;
        let job = Job {
            owner,
            work,
            wait,
            elapsed: 0.0,
            born,
        };
        self.jobs.insert(handle, job);
    }

    pub(crate) fn rearm(&mut self, handle: u64, wait: Wait, carry: f64) {
        let epoch = self.epoch;
        if let Some(job) = self.jobs.get_mut(&handle) {
            job.wait = wait;
            job.elapsed = carry;
            job.born = epoch;
        }
    }

    pub(crate) fn get(&self, handle: u64) -> Option<&Job> {
        self.jobs.get(&handle)
    }

    pub(crate) fn cancel(&mut self, handle: u64) -> bool {
        self.jobs.remove(&handle).is_some()
    }

    pub(crate) fn retain_owners(&mut self, keep: impl Fn(u32) -> bool) {
        self.jobs.retain(|_, job| keep(job.owner));
    }

    pub(crate) fn cancel_invokes(&mut self, owner: u32, name: Option<&str>) {
        self.jobs
            .retain(|_, job| !(job.owner == owner && invoke_matches(&job.work, name)));
    }

    pub(crate) fn cancel_coroutines(&mut self, owner: u32) {
        self.jobs
            .retain(|_, job| !(job.owner == owner && matches!(job.work, Work::Coroutine { .. })));
    }

    pub(crate) fn is_invoking(&self, owner: u32, name: Option<&str>) -> bool {
        self.jobs
            .values()
            .any(|job| job.owner == owner && invoke_matches(&job.work, name))
    }

    pub(crate) fn clear(&mut self) {
        self.jobs.clear();
    }

    pub(crate) fn advance(&mut self, scaled: f64, unscaled: f64) -> Vec<u64> {
        let mut due = Vec::new();
        for (&handle, job) in &mut self.jobs {
            if job.born == self.epoch {
                continue;
            }
            let ready = match &job.wait {
                Wait::Elapsed { delay, clock } => {
                    job.elapsed += match clock {
                        Clock::Scaled => scaled,
                        Clock::Unscaled => unscaled,
                    };
                    job.elapsed + ELAPSED_EPSILON >= *delay
                }
                Wait::Until(_) => true,
            };
            if ready {
                due.push((job.owner, handle));
            }
        }
        due.sort_unstable();
        due.into_iter().map(|(_, handle)| handle).collect()
    }

    pub(crate) fn end_phase(&mut self) {
        self.epoch += 1;
    }

    pub(crate) fn overshoot(&self, handle: u64) -> f64 {
        match self.jobs.get(&handle) {
            Some(Job {
                wait: Wait::Elapsed { delay, .. },
                elapsed,
                ..
            }) => (elapsed - delay).max(0.0),
            _ => 0.0,
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

fn invoke_matches(work: &Work, name: Option<&str>) -> bool {
    match (work, name) {
        (Work::Invoke { .. }, None) => true,
        (
            Work::Invoke {
                target: Target::Name(n),
                ..
            },
            Some(want),
        ) => n == want,
        _ => false,
    }
}
