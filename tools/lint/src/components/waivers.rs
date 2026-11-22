pub(super) const WAIVERS: &[(&str, &str, &str)] = &[
    (
        "collider",
        "api",
        "Served by the `Physics` namespace: the collider is queried via \
         Physics.Raycast plus the #311 spatial surface (Overlap*/Check*, \
         SphereCast, ClosestPoint/ContainsPoint, GetBounds) — the same rapier \
         world the engine casts against — and its shape and physics material \
         are authored through Physics.Get/SetColliderShape and \
         Physics.Get/SetPhysicsMaterial (#447). A separate `Collider` namespace would \
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
        "Served by the `NavMeshAgent`/`Navigation` namespace in src/script/api/nav/mod.rs \
         (the field is `nav_agent`, the namespace is the Unity name `NavMeshAgent`).",
    ),
    (
        "post_processing",
        "api",
        "Served by the `Graphics` namespace, which drives the active \
         PostProcessingComponent's bloom/SSR/tonemap/exposure knobs (render-only \
         state). A per-component namespace would duplicate that surface.",
    ),
];

pub(super) fn waived(field: &str, axis: &str) -> bool {
    WAIVERS.iter().any(|(c, a, _)| *c == field && *a == axis)
}

pub(super) fn stale_waivers(
    components: &[String],
    present: impl Fn(&str, &str) -> bool,
) -> Vec<String> {
    WAIVERS
        .iter()
        .filter_map(|&(c, a, _)| {
            let why = if !components.iter().any(|f| f == c) {
                "is not a discovered component"
            } else if present(c, a) {
                "axis is present"
            } else {
                return None;
            };
            Some(format!("STALE_WAIVER `{c}` `{a}`: {why} — delete the row"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waivers_are_recognized_and_scoped() {
        for (c, a, rationale) in WAIVERS {
            assert!(waived(c, a), "{c}/{a} should be waived");
            assert!(!rationale.is_empty(), "{c}/{a} needs a rationale");
        }
        assert!(waived("collider", "api"));
        assert!(!waived("collider", "inspector"));
        assert!(!waived("particles", "api"));
    }

    #[test]
    fn stale_waivers_are_flagged() {
        let all: Vec<String> = WAIVERS.iter().map(|(c, _, _)| c.to_string()).collect();
        assert!(stale_waivers(&all, |_, _| false).is_empty());
        let met = stale_waivers(&all, |c, a| c == "collider" && a == "api");
        assert_eq!(met.len(), 1);
        assert!(met[0].starts_with("STALE_WAIVER `collider` `api`"));
        let gone: Vec<String> = all.iter().filter(|c| *c != "rigidbody").cloned().collect();
        let stale = stale_waivers(&gone, |_, _| false);
        assert_eq!(stale.len(), 1);
        assert!(stale[0].contains("`rigidbody`"));
    }

    #[test]
    fn light_api_is_not_waived_it_is_implemented() {
        assert!(!waived("light", "api"));
    }
}
