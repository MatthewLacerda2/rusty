//! The active scene's probes are `Scene::probes` and `Scene::reflection_probes`; the others
//! wait on their entries, moved, never cloned, so a set keeps its revision.

use glam::{Vec3, Vec4};

use crate::lighting::probe::{ProbeField, ProbeVolume};
use crate::lighting::reflection_probe::{ReflectionProbe, ReflectionProbeSet};
use crate::lighting::sh::Sh9;
use crate::runtime::membership::Member;
use crate::Scene;

#[derive(Clone, Debug, Default)]
pub struct SceneProbes {
    pub light: ProbeVolume,
    pub reflection: ReflectionProbeSet,
}

impl Scene {
    fn own_handle(&self) -> u32 {
        self.active_handle().unwrap_or_default()
    }

    pub fn in_active_scene(&self, id: u32) -> bool {
        self.member_of(id) == Some(Member::Scene(self.active_handle()))
    }

    pub fn probe_sets(&self) -> Vec<(u32, &ProbeVolume)> {
        self.in_load_order(&self.probes, |p| &p.light)
    }

    pub fn with_probe_field<R>(&self, f: impl FnOnce(ProbeField<'_>) -> R) -> R {
        let sets = self.probe_sets();
        f(ProbeField::new(&sets))
    }

    pub fn sample_probes(&self, position: Vec3) -> Option<Sh9> {
        match self.loaded.scenes.is_empty() {
            true => self.probes.sample(position),
            false => self.with_probe_field(|f| f.sample(position)),
        }
    }

    pub fn sample_probe_occlusion(&self, position: Vec3) -> Option<Vec4> {
        match self.loaded.scenes.is_empty() {
            true => self.probes.sample_occlusion(position),
            false => self.with_probe_field(|f| f.sample_occlusion(position)),
        }
    }

    pub fn probes_empty(&self) -> bool {
        self.probe_sets().iter().all(|(_, v)| v.is_empty())
    }

    pub fn all_reflection_probes(&self) -> Vec<&ReflectionProbe> {
        let sets = self.in_load_order(&self.reflection_probes, |p| &p.reflection);
        sets.into_iter().flat_map(|(_, s)| &s.probes).collect()
    }

    fn in_load_order<'a, T>(
        &'a self,
        own: &'a T,
        of: impl Fn(&'a SceneProbes) -> &'a T,
    ) -> Vec<(u32, &'a T)> {
        let handle = self.own_handle();
        let others = self.loaded.scenes.iter().filter(|s| s.handle != handle);
        let parked = others.filter_map(|s| Some((s.handle, of(s.parked.as_ref()?))));
        let mut sets: Vec<(u32, &T)> = parked.collect();
        let at = sets.partition_point(|(h, _)| *h < handle);
        sets.insert(at, (handle, own));
        sets
    }

    pub fn park_probes(&mut self, handle: u32, probes: SceneProbes) {
        if let Some(entry) = self.loaded.entry(handle) {
            entry.parked = Some(probes);
        }
    }

    pub(super) fn swap_probes(&mut self, old: u32, new: u32) {
        let held = SceneProbes {
            light: std::mem::take(&mut self.probes),
            reflection: std::mem::take(&mut self.reflection_probes),
        };
        if let Some(entry) = self.loaded.entry(old) {
            entry.parked = Some(held);
        }
        let next = self.loaded.entry(new).and_then(|e| e.parked.take());
        let next = next.unwrap_or_default();
        self.probes = next.light;
        self.reflection_probes = next.reflection;
    }
}

#[cfg(test)]
#[path = "probe_sets_tests.rs"]
mod tests;
