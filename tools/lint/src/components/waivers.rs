//! Deliberate-waiver table (#82): axes intentionally served by a shared namespace
//! or a content-driven workflow, each row carrying its written rationale.

/// Deliberately-waived axes: `(component_field, axis, rationale)`. A waived axis
/// is treated as satisfied. Each row is a documented decision (#82) NOT to add a
/// per-component artifact, because the axis is already served another way and
/// doing so standalone would fragment the one stable API surface or duplicate a
/// content-driven workflow. Reviewable here, never a silent skip.
pub(super) const WAIVERS: &[(&str, &str, &str)] = &[
    (
        "mesh",
        "add_menu",
        "Mesh is assigned by dragging an asset from the content grid / the render \
         inspector, not picked from the Add Component menu — a blank mesh slot is \
         meaningless. Authoring stays content-driven (CLAUDE.md: glTF/OBJ sources).",
    ),
    (
        "mesh",
        "api",
        "Mesh geometry is content (glTF/OBJ), not a scriptable scalar surface; \
         swapping meshes at runtime is out of the script API's scope. Material \
         look is driven via the `Material` namespace instead.",
    ),
    (
        "material",
        "api",
        "Served by the `Material` namespace (SetTexture/SetMetallic/SetRoughness \
         and their map variants) — the `material` field is a reference to a shared \
         library material, and a dedicated `Material`-field namespace would just \
         restate the same surface.",
    ),
    (
        "collider",
        "api",
        "Served by the `Physics` namespace: the collider is queried via \
         Physics.Raycast plus the #311 spatial surface (Overlap*/Check*, \
         SphereCast, ClosestPoint/ContainsPoint, GetBounds) — the same rapier \
         world the engine casts against. A separate `Collider` namespace would \
         split physics across two surfaces.",
    ),
    (
        "rigidbody",
        "api",
        "Served by the `Physics` namespace \
         (GetVelocity/SetVelocity/AddForce/SetKinematic act on the rigidbody).",
    ),
    (
        "nav_agent",
        "api",
        "Served by the `NavMeshAgent`/`Navigation` namespace in src/api/nav.rs \
         (the field is `nav_agent`, the namespace is the Unity name `NavMeshAgent`).",
    ),
    (
        "visual_correction",
        "api",
        "Served by the `Graphics` namespace, which drives the active \
         VisualCorrectionComponent's bloom/SSR/tonemap/exposure knobs (render-only \
         state). A per-component namespace would duplicate that surface.",
    ),
];

/// True when `(field, axis)` is a documented [`WAIVERS`] decision — served by a
/// shared namespace or a content-driven workflow, not to be implemented standalone.
pub(super) fn waived(field: &str, axis: &str) -> bool {
    WAIVERS.iter().any(|(c, a, _)| *c == field && *a == axis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waivers_are_recognized_and_scoped() {
        // Every waiver row is recognized for its own (component, axis)…
        for (c, a, rationale) in WAIVERS {
            assert!(waived(c, a), "{c}/{a} should be waived");
            assert!(!rationale.is_empty(), "{c}/{a} needs a rationale");
        }
        // …and a waiver does not leak to a different axis of the same component.
        assert!(waived("collider", "api"));
        assert!(!waived("collider", "inspector"));
        // A component with no waiver (e.g. particles) is never waived.
        assert!(!waived("particles", "api"));
    }

    #[test]
    fn light_api_is_not_waived_it_is_implemented() {
        // `light` gets a real `src/api/light.rs` namespace (#82), so it must NOT be
        // on the waiver list — removing the namespace must make the gate fail.
        assert!(!waived("light", "api"));
    }
}
