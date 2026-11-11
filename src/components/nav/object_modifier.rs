//! src/components/nav/object_modifier.rs — NavMeshModifier (#1959).
//!
//! Unity's (AI Navigation's) `NavMeshModifier`: a per-object say in how the navmesh
//! bake reads this entity's colliders, and by default its children's.
//!
//! * `override_area`: the colliders bake as that area. `NotWalkable` (`1`) makes a
//!   tabletop or a sofa seat unwalkable, while the object stays solid geometry, so
//!   it still blocks the floor beneath it through the agent's clearance, as any
//!   geometry does. Furniture wants this.
//! * `ignore_from_build`: the colliders are left out of the bake entirely, so the
//!   object neither carries a surface nor blocks anything (the floor under it is
//!   walkable).
//! * `apply_to_children`: the modifier also covers the entity's descendants; a
//!   descendant's own modifier wins over its ancestor's.
//!
//! Unlike a `NavMeshModifierVolume`, it follows the colliders' own geometry, so it
//! needs no hand-sized box and moves with the object.

use serde::{Deserialize, Serialize};

/// A per-object navmesh modifier. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NavMeshModifierComponent {
    /// An inactive modifier changes nothing, and does not shield a child from an
    /// ancestor's.
    pub active: bool,
    /// The area id this object's walkable tops bake as, `None` to keep the bake's.
    pub override_area: Option<u8>,
    /// Leave this object's colliders out of the bake.
    pub ignore_from_build: bool,
    /// Cover the entity's descendants too.
    pub apply_to_children: bool,
}

impl Default for NavMeshModifierComponent {
    /// Unity's defaults: active, no override, built, applied to children.
    fn default() -> Self {
        Self {
            active: true,
            override_area: None,
            ignore_from_build: false,
            apply_to_children: true,
        }
    }
}

impl crate::components::FirstClass for NavMeshModifierComponent {
    const NAME: &'static str = "NavMeshModifier";
    const ALIASES: &'static [&'static str] = &["navmodifier"];
}
