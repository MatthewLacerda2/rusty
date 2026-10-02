//! Crossing an off-mesh link (#462): `Walking → OnLink → Walking`.
//!
//! When an agent reaches the start of a link on its path, it stops steering and is
//! **on the link** (Unity's `isOnOffMeshLink`), exposing the link to its script.
//! With auto-traverse on (the default, Unity's `autoTraverseOffMeshLink`) the
//! engine carries it to the link's end in a straight line at its speed; with it
//! off, the agent waits — the script plays the vault, jump or climb, moves the
//! body, and calls `CompleteOffMeshLink`, which puts the agent on the link's end
//! and resumes the path. Frame-count driven like the rest of the tick, so a
//! traversal replays bit for bit.

use glam::Vec3;

use crate::scene::NavMeshAgentComponent;

/// Put the agent on the link whose leg leads into the waypoint its cursor just
/// reached, if there is one. Returns whether it is now on a link.
pub(super) fn enter_link(agent: &mut NavMeshAgentComponent) -> bool {
    let cursor = agent.path_cursor;
    let Some(&(_, link)) = agent.path_links.iter().find(|(w, _)| *w == cursor) else {
        return false;
    };
    agent.off_mesh_link = Some(link);
    agent.velocity = Vec3::ZERO;
    true
}

/// One tick of the engine's own traversal: `pos` moved toward the link's end by
/// `speed · dt`. Arriving completes the link.
pub(super) fn auto_traverse(agent: &mut NavMeshAgentComponent, pos: Vec3, dt: f32) -> Vec3 {
    let Some(link) = agent.off_mesh_link else {
        return pos;
    };
    let to_end = link.end - pos;
    let step = agent.speed.max(0.0) * dt;
    if to_end.length() <= step {
        return complete_off_mesh_link(agent).unwrap_or(link.end);
    }
    pos + to_end.normalize_or_zero() * step
}

/// Finish the link the agent is on (Unity's `CompleteOffMeshLink`): it leaves the
/// link at rest and walks on from its end, which is returned for the caller to
/// place the agent at. `None` when it is not on a link.
pub fn complete_off_mesh_link(agent: &mut NavMeshAgentComponent) -> Option<Vec3> {
    let link = agent.off_mesh_link.take()?;
    agent.velocity = Vec3::ZERO;
    Some(link.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{OffMeshLinkData, OffMeshLinkKind};

    fn on_link() -> NavMeshAgentComponent {
        let link = OffMeshLinkData {
            kind: OffMeshLinkKind::Drop,
            start: Vec3::ZERO,
            end: Vec3::new(0.0, -3.0, 4.0),
            owner: None,
        };
        NavMeshAgentComponent {
            speed: 2.5,
            cached_path: vec![Vec3::ZERO, link.end],
            path_links: vec![(1, link)],
            ..Default::default()
        }
    }

    #[test]
    fn reaching_a_link_start_enters_it() {
        let mut a = on_link();
        assert!(!enter_link(&mut a), "waypoint 0 is no link's end");
        a.path_cursor = 1;
        assert!(enter_link(&mut a));
        assert_eq!(a.off_mesh_link.map(|l| l.kind), Some(OffMeshLinkKind::Drop));
    }

    #[test]
    fn auto_traverse_walks_the_link_at_speed_then_completes() {
        let mut a = on_link();
        a.path_cursor = 1;
        enter_link(&mut a);
        let mut pos = Vec3::ZERO;
        let mut ticks = 0;
        while a.off_mesh_link.is_some() {
            pos = auto_traverse(&mut a, pos, 0.1);
            ticks += 1;
        }
        assert_eq!(pos, Vec3::new(0.0, -3.0, 4.0), "lands on the end");
        assert_eq!(ticks, 20, "5 m at 2.5 m/s in 0.1 s ticks");
    }

    #[test]
    fn completing_off_a_link_is_a_no_op() {
        let mut a = on_link();
        assert_eq!(complete_off_mesh_link(&mut a), None);
    }
}
