use super::ReflectionProbeSet;
use crate::Scene;

impl Scene {
    pub fn inspect_reflection_probe(&mut self, index: usize) -> Result<(), String> {
        let count = self.reflection_probes.len();
        if index >= count {
            return Err(format!(
                "no reflection probe {index}: the scene has {count} (0-based)"
            ));
        }
        self.inspected_reflection_probe = Some(index);
        Ok(())
    }

    pub fn remove_reflection_probe(&mut self, index: usize) {
        if index < self.reflection_probes.len() {
            self.inspected_reflection_probe = None;
        }
        self.reflection_probes.remove(index);
    }

    pub fn replace_reflection_probes(&mut self, set: ReflectionProbeSet) {
        let at = |s: &ReflectionProbeSet, i: usize| s.probes.get(i).map(|p| p.position);
        let kept = |&i: &usize| at(&set, i) == at(&self.reflection_probes, i);
        self.inspected_reflection_probe = self.inspected_reflection_probe.filter(kept);
        self.reflection_probes = set;
    }

    pub fn clear_reflection_probes(&mut self) {
        self.reflection_probes.clear();
        self.inspected_reflection_probe = None;
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use crate::Scene;

    fn three() -> Scene {
        let mut scene = Scene::new();
        for x in [0.0, 5.0, 10.0] {
            scene.reflection_probes.add(Vec3::X * x, Vec3::ONE);
        }
        scene
    }

    #[test]
    fn an_index_past_the_probes_is_refused_and_changes_nothing() {
        let mut scene = three();
        assert_eq!(scene.inspect_reflection_probe(1), Ok(()));
        let err = scene.inspect_reflection_probe(3).unwrap_err();
        assert_eq!(err, "no reflection probe 3: the scene has 3 (0-based)");
        assert_eq!(scene.inspected_reflection_probe, Some(1));
    }

    #[test]
    fn removing_a_probe_leaves_none_inspected_in_its_place() {
        for (inspected, removed) in [(1, 1), (2, 0), (0, 2)] {
            let mut scene = three();
            scene.inspect_reflection_probe(inspected).unwrap();
            scene.remove_reflection_probe(removed);
            assert_eq!(
                scene.inspected_reflection_probe, None,
                "{inspected}, {removed}"
            );
        }
        let mut scene = three();
        scene.inspect_reflection_probe(1).unwrap();
        scene.remove_reflection_probe(7);
        assert_eq!(scene.inspected_reflection_probe, Some(1), "nothing removed");
        scene.clear_reflection_probes();
        assert_eq!(scene.inspected_reflection_probe, None);
    }

    #[test]
    fn a_new_placement_keeps_the_inspected_probe_only_where_it_stayed() {
        let mut scene = three();
        scene.inspect_reflection_probe(1).unwrap();
        let mut set = scene.reflection_probes.clone();
        set.probes[0].position = Vec3::Y;
        scene.replace_reflection_probes(set.clone());
        assert_eq!(scene.inspected_reflection_probe, Some(1), "probe 1 stayed");
        set.probes[1].position = Vec3::Y;
        scene.replace_reflection_probes(set);
        assert_eq!(scene.inspected_reflection_probe, None, "probe 1 moved");
    }
}
