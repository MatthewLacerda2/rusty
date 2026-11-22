use std::collections::BTreeMap;

use serde::Serialize;

use crate::lighting::probe::ProbeVolume;
use crate::lighting::reflection_probe::ReflectionProbeSet;
use crate::mesh_library::put_back as put_meshes;
use crate::Scene;

macro_rules! scene_level {
    (authored { $($field:ident: $ty:ty => $same:ident, $put:ident;)* }) => {
        #[derive(Clone, Debug)]
        pub(super) struct SceneLevel {
            $($field: $ty,)*
        }

        /// Only the fields a step changed, so its Undo never reverts one it did not touch.
        #[derive(Debug)]
        pub(super) struct LevelDelta {
            $($field: Option<$ty>,)*
        }

        impl SceneLevel {
            pub(super) fn capture(scene: &Scene) -> Self {
                Self { $($field: scene.$field.clone(),)* }
            }

            pub(super) fn matches(&self, scene: &Scene) -> bool {
                true $(&& $same(&self.$field, &scene.$field))*
            }

            pub(super) fn delta(self, scene: &Scene) -> LevelDelta {
                LevelDelta {
                    $($field: (!$same(&self.$field, &scene.$field)).then_some(self.$field),)*
                }
            }
        }

        impl LevelDelta {
            pub(super) fn inverse(&self, scene: &Scene) -> Self {
                Self { $($field: self.$field.as_ref().map(|_| scene.$field.clone()),)* }
            }

            pub(super) fn restore(self, scene: &mut Scene) {
                $(if let Some(value) = self.$field {
                    $put(value, &mut scene.$field);
                })*
            }

            /// This step's record of a field is older, so it wins.
            pub(super) fn absorb(&mut self, later: Self) {
                $(if self.$field.is_none() {
                    self.$field = later.$field;
                })*
            }
        }
    };
}

crate::state::inventory::authored_fields!(scene_level {});

fn eq<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

/// `==` first; on a mismatch the saved form decides, so a NaN is no edit.
fn eq_or_bytes<T: PartialEq + Serialize>(a: &T, b: &T) -> bool {
    a == b || same_bytes(a, b)
}

fn same_bytes<T: Serialize>(a: &T, b: &T) -> bool {
    serde_json::to_vec(a).ok() == serde_json::to_vec(b).ok()
}

fn assign<T>(value: T, slot: &mut T) {
    *slot = value;
}

fn same_probes(a: &ProbeVolume, b: &ProbeVolume) -> bool {
    a.grid() == b.grid()
        && a.probes().len() == b.probes().len()
        && a.probes()
            .iter()
            .zip(b.probes())
            .all(|(p, q)| eq_or_bytes(&p.position, &q.position) && p.manual == q.manual)
}

fn at(p: glam::Vec3) -> [u32; 3] {
    p.to_array().map(f32::to_bits)
}

/// Each keeps the baked SH and occlusion of a live probe at its position: by position, not
/// index, since a bake that re-places the set would hand old probes the new set's light.
fn put_probes(mut value: ProbeVolume, slot: &mut ProbeVolume) {
    let mut live: BTreeMap<[u32; 3], _> = BTreeMap::new();
    for p in slot.probes().iter().rev() {
        live.insert(at(p.position), (p.sh, p.occlusion));
    }
    for p in value.probes_mut() {
        if let Some(&(sh, occlusion)) = live.get(&at(p.position)) {
            p.sh = sh;
            p.occlusion = occlusion;
        }
    }
    *slot = value;
}

fn same_reflections(a: &ReflectionProbeSet, b: &ReflectionProbeSet) -> bool {
    a.probes.len() == b.probes.len()
        && a.probes.iter().zip(&b.probes).all(|(p, q)| {
            let unbaked = |r: &crate::lighting::reflection_probe::ReflectionProbe| {
                let mut r = r.clone();
                r.cubemap_path.clear();
                r.irradiance = None;
                r
            };
            p == q || eq_or_bytes(&unbaked(p), &unbaked(q))
        })
}

/// Each keeps the cubemap and irradiance of a live probe at its position, as `put_probes`.
fn put_reflections(mut value: ReflectionProbeSet, slot: &mut ReflectionProbeSet) {
    let mut live: BTreeMap<[u32; 3], (&str, _)> = BTreeMap::new();
    for p in slot.probes.iter().rev() {
        live.insert(at(p.position), (p.cubemap_path.as_str(), p.irradiance));
    }
    for p in &mut value.probes {
        if let Some(&(path, irradiance)) = live.get(&at(p.position)) {
            p.cubemap_path = path.to_owned();
            p.irradiance = irradiance;
        }
    }
    *slot = value;
}
