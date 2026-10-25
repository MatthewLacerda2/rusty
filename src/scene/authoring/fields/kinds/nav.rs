//! src/scene/authoring/fields/kinds/nav.rs — the navigation components' field tables.

use super::super::table::{inspect, op, ro, with};
use super::super::value::Mask;
use crate::components::{
    NavMeshAgentComponent, NavMeshModifierVolumeComponent, NavMeshObstacleComponent,
    OffMeshLinkComponent,
};
use crate::scene::authoring::nav::offmesh_link as link;
use crate::scene::authoring::nav::{agent, modifier, obstacle};

inspect! {
    NavMeshAgentComponent => nav_agent, nav_agent_mut, &[
        op!("active", agent::set_active),
        op!("radius", agent::set_radius),
        op!("target", agent::set_target),
        op!("speed", agent::set_speed),
        op!("acceleration", agent::set_acceleration),
        op!("stopping_distance", agent::set_stopping_distance),
        ro!("velocity", "the agent's steering writes it every tick"),
        op!("avoidance_priority", agent::set_avoidance_priority),
        op!("avoidance_enabled", agent::set_avoidance_enabled),
        op!("auto_traverse_off_mesh_link", agent::set_auto_traverse_off_mesh_link),
        op!("base_offset", agent::set_base_offset),
        with!("area_mask", |a, v: Mask| agent::set_area_mask(a, v.0)),
        op!("update_rotation", agent::set_update_rotation),
        op!("angular_speed", agent::set_angular_speed),
        op!("angular_acceleration", agent::set_angular_acceleration),
    ];
    NavMeshObstacleComponent => nav_obstacle, nav_obstacle_mut, &[
        op!("active", obstacle::set_active),
        op!("shape", obstacle::set_shape),
        op!("center", obstacle::set_center),
        op!("size", obstacle::set_size),
        op!("radius", obstacle::set_radius),
        op!("height", obstacle::set_height),
        op!("carve", obstacle::set_carve),
        op!("carve_only_stationary", obstacle::set_carve_only_stationary),
        op!("move_threshold", obstacle::set_move_threshold),
        op!("time_to_stationary", obstacle::set_time_to_stationary),
    ];
    OffMeshLinkComponent => offmesh_link, offmesh_link_mut, &[
        op!("active", link::set_active),
        op!("start", link::set_start),
        op!("end", link::set_end),
        op!("bidirectional", link::set_bidirectional),
        op!("cost_override", link::set_cost_override),
        op!("area", link::set_area),
    ];
    NavMeshModifierVolumeComponent => nav_modifier, nav_modifier_mut, &[
        op!("active", modifier::set_active),
        op!("center", modifier::set_center),
        op!("size", modifier::set_size),
        op!("area", modifier::set_area),
    ];
}
