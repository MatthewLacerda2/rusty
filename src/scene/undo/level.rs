//! src/scene/undo/level.rs — the scene-level "object" an undo step records (#1631).
//!
//! Scene Settings, fog, layers, the collision matrix, the material library, the
//! probe sets and the bake's settings live on `Scene`, beside the World, and are
//! written by plain field assignment from the inspector and the API alike. They
//! are recorded as one more object: the stack keeps a copy as a baseline, and an
//! edit that leaves the scene differing from it records the fields it changed, as
//! they were before ([`LevelDelta`]). One list of fields, so the copy, the comparison and the restore never
//! disagree about what the scene level is, and that list is the scene state
//! inventory's authored fields (`scene::state::inventory`, #1649): a field is
//! undoable because it is classified authored, not because this file names it.
//!
//! **Bake outputs are not edits** (Unity's Undo never reverts a lighting bake):
//! the lightmaps are not on the list at all, and of the probe sets only what the
//! user places is: a light probe's baked SH and a reflection probe's cubemap are
//! neither compared nor put back while the set keeps its length, so an Undo of a
//! probe move after a bake keeps the bake. A restore that changes a set's length
//! (an add or a removal undone) takes the recorded probes whole.

use serde::Serialize;

use crate::scene::lighting::probe::ProbeVolume;
use crate::scene::lighting::reflection_probe::ReflectionProbeSet;
use crate::scene::Scene;

macro_rules! scene_level {
    (authored { $($field:ident: $ty:ty => $same:ident, $put:ident;)* }) => {
        /// The scene-level state, as values: the open edit's baseline.
        #[derive(Clone, Debug)]
        pub(super) struct SceneLevel {
            $($field: $ty,)*
        }

        /// The fields one step changed, each as it was before: what it puts back.
        /// Only those, so undoing a setting never reverts a field the step did
        /// not touch (a bake's probes placed since, a later edit's fog).
        #[derive(Debug)]
        pub(super) struct LevelDelta {
            $($field: Option<$ty>,)*
        }

        impl SceneLevel {
            /// `scene`'s scene-level state, copied.
            pub(super) fn capture(scene: &Scene) -> Self {
                Self { $($field: scene.$field.clone(),)* }
            }

            /// Whether `scene`'s scene level is still this one, as an edit sees it.
            pub(super) fn matches(&self, scene: &Scene) -> bool {
                true $(&& $same(&self.$field, &scene.$field))*
            }

            /// The fields `scene` moved away from this baseline, as they were.
            pub(super) fn delta(self, scene: &Scene) -> LevelDelta {
                LevelDelta {
                    $($field: (!$same(&self.$field, &scene.$field)).then_some(self.$field),)*
                }
            }
        }

        impl LevelDelta {
            /// The same fields as `scene` holds them now: the step that redoes this.
            pub(super) fn inverse(&self, scene: &Scene) -> Self {
                Self { $($field: self.$field.as_ref().map(|_| scene.$field.clone()),)* }
            }

            /// Put these fields back on `scene`.
            pub(super) fn restore(self, scene: &mut Scene) {
                $(if let Some(value) = self.$field {
                    $put(value, &mut scene.$field);
                })*
            }

            /// Fold a later step's fields in; this step's older record of a field wins.
            pub(super) fn absorb(&mut self, later: Self) {
                $(if self.$field.is_none() {
                    self.$field = later.$field;
                })*
            }
        }
    };
}

// The scene level is the inventory's authored fields (`scene::state`, #1649).
crate::scene::state::inventory::authored_fields!(scene_level {});

fn eq<T: PartialEq>(a: &T, b: &T) -> bool {
    a == b
}

/// `==` first (cheap); on a mismatch, the saved form decides, so a NaN that
/// never equals itself is no edit.
fn eq_or_bytes<T: PartialEq + Serialize>(a: &T, b: &T) -> bool {
    a == b || same_bytes(a, b)
}

/// Whether two values save the same. Only `==`'s fallback: serializing the
/// material library every frame costs a Dust2-sized scene ~60 µs.
fn same_bytes<T: Serialize>(a: &T, b: &T) -> bool {
    serde_json::to_vec(a).ok() == serde_json::to_vec(b).ok()
}

fn assign<T>(value: T, slot: &mut T) {
    *slot = value;
}

/// The probe set as placed: the grid and every position, not the baked SH.
fn same_probes(a: &ProbeVolume, b: &ProbeVolume) -> bool {
    a.grid == b.grid
        && a.probes.len() == b.probes.len()
        && a.probes
            .iter()
            .zip(&b.probes)
            .all(|(p, q)| eq_or_bytes(&p.position, &q.position))
}

/// Put the probes back, keeping the live baked SH while the count is unchanged.
fn put_probes(mut value: ProbeVolume, slot: &mut ProbeVolume) {
    if value.probes.len() == slot.probes.len() {
        for (p, live) in value.probes.iter_mut().zip(&slot.probes) {
            p.sh = live.sh;
        }
    }
    *slot = value;
}

/// The reflection probes as placed and set, without the baked cubemap's path.
fn same_reflections(a: &ReflectionProbeSet, b: &ReflectionProbeSet) -> bool {
    a.probes.len() == b.probes.len()
        && a.probes.iter().zip(&b.probes).all(|(p, q)| {
            let unbaked = |r: &crate::scene::lighting::reflection_probe::ReflectionProbe| {
                let mut r = r.clone();
                r.cubemap_path.clear();
                r
            };
            p == q || eq_or_bytes(&unbaked(p), &unbaked(q))
        })
}

/// Put the reflection probes back, keeping the live cubemaps while the count is
/// unchanged.
fn put_reflections(mut value: ReflectionProbeSet, slot: &mut ReflectionProbeSet) {
    if value.probes.len() == slot.probes.len() {
        for (p, live) in value.probes.iter_mut().zip(&mut slot.probes) {
            p.cubemap_path = std::mem::take(&mut live.cubemap_path);
        }
    }
    *slot = value;
}
