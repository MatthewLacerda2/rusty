use glam::{Vec2, Vec3};

use super::super::avoidance::{self, AvoidanceAgent};
use super::super::obstacle::avoidance_obstacles;
use super::super::{path_length, NavigationGraph};
use super::link::auto_traverse;
use super::root_motion::root_moved;
use super::state::is_at_target;
use super::turn::turn_toward;
use rusty_scene::{NavMeshAgentComponent, Scene};

struct Ticked {
    id: u32,
    position: Vec3,
    moving: bool,
}

fn xz(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

impl NavigationGraph {
    /// Visits agents in the ECS's stable order: the tick is deterministic.
    pub fn tick_nav_agents(&self, scene: &mut Scene, delta_time: f32) {
        self.ask_paths(scene);
        self.serve_paths(scene);
        let mut ticked = Vec::new();
        let mut inputs = Vec::new();
        for id in scene.world.ids_with_nav_agent() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            // Held outside its column so World calls never alias it.
            let Some(mut agent) = hold_agent(scene, id) else {
                continue;
            };
            let position = scene.world.world_position(id);
            if agent.active {
                if agent.off_mesh_link.is_some() {
                    self.on_link(scene, id, agent, position, delta_time);
                    continue;
                }
                let steering = !agent.cached_path.is_empty() && !is_at_target(&agent, position);
                let moved_with = agent.velocity;
                let heading = self.steer_or_brake(&mut agent, position, steering, delta_time);
                let mut facing = scene.world.world_rotation(id);
                turn_toward(&mut agent, &mut facing, heading, delta_time);
                scene.world.set_world_rotation(id, facing);
                let root = root_moved(scene, id, delta_time);
                let mut input = avoidance_input(&agent, position, moved_with, steering);
                if let Some(v) = root {
                    (input.velocity, input.preferred, input.solves) = (xz(v), xz(v), false);
                }
                inputs.push(input);
                ticked.push(Ticked {
                    id,
                    position,
                    moving: (steering || agent.velocity != Vec3::ZERO) && root.is_none(),
                });
            }
            put_back(scene, id, agent);
        }

        // Obstacles follow the agents; zipping with `ticked` stops before them.
        inputs.extend(avoidance_obstacles(scene));
        let velocities = avoidance::solve(&inputs, delta_time);
        for (t, v) in ticked.iter().zip(velocities) {
            if t.moving {
                self.integrate(scene, t, Vec3::new(v.x, 0.0, v.y), delta_time);
            }
        }
    }

    fn steer_or_brake(
        &self,
        agent: &mut NavMeshAgentComponent,
        position: Vec3,
        steering: bool,
        delta_time: f32,
    ) -> Vec2 {
        if steering {
            return self.accelerate_toward_waypoint(agent, position, delta_time);
        }
        decelerate(agent, delta_time);
        Vec2::ZERO
    }

    /// `acceleration` in m/s²; brakes at that rate (Unity's `autoBraking`).
    fn accelerate_toward_waypoint(
        &self,
        agent: &mut NavMeshAgentComponent,
        current_pos: Vec3,
        delta_time: f32,
    ) -> Vec2 {
        let feet = agent.feet(current_pos);
        let next_step = self.cached_next_step(agent, feet);
        if agent.off_mesh_link.is_some() {
            return Vec2::ZERO;
        }
        // Steer on XZ so a tall step doesn't inflate the move distance.
        let mut to_next_dir = next_step - current_pos;
        to_next_dir.y = 0.0;
        let to_next_dir = to_next_dir.normalize_or_zero();

        let room = (path_left(agent, feet, next_step) - agent.stopping_distance).max(0.0);
        let speed = agent
            .speed
            .min(braking_speed(agent.acceleration, room, delta_time));
        let desired = xz(to_next_dir * speed);
        let steered = move_towards(xz(agent.velocity), desired, agent.acceleration * delta_time);
        agent.velocity = Vec3::new(steered.x, 0.0, steered.y);
        xz(to_next_dir)
    }

    fn on_link(
        &self,
        scene: &mut Scene,
        id: u32,
        mut agent: NavMeshAgentComponent,
        position: Vec3,
        delta_time: f32,
    ) {
        agent.angular_velocity = 0.0;
        let moved = agent
            .auto_traverse_off_mesh_link
            .then(|| auto_traverse(&mut agent, position, delta_time));
        put_back(scene, id, agent);
        if let Some(p) = moved {
            scene.world.set_world_position(id, p);
        }
    }

    fn integrate(&self, scene: &mut Scene, t: &Ticked, velocity: Vec3, delta_time: f32) {
        let new_pos = {
            let Some(mut agent) = scene.world.nav_agent_mut(t.id) else {
                return;
            };
            agent.velocity = velocity;
            self.slide(&mut agent, t.position, delta_time)
        };
        scene.world.set_world_position(t.id, new_pos);
    }

    /// Stands on the span it reached, not the column's highest floor.
    pub(super) fn slide(
        &self,
        agent: &mut NavMeshAgentComponent,
        body: Vec3,
        delta_time: f32,
    ) -> Vec3 {
        let current_pos = agent.feet(body);
        let Some(from) = self.span_under(current_pos) else {
            agent.velocity = Vec3::ZERO;
            return body;
        };
        let mut final_pos = current_pos;
        let mut on = from;

        let proposed_x = current_pos + Vec3::new(agent.velocity.x * delta_time, 0.0, 0.0);
        let (gx, gz) = self.world_to_grid(proposed_x);
        match self.link_to(from, gx, gz) {
            Some(s) => {
                final_pos.x = proposed_x.x;
                on = s;
            }
            None => agent.velocity.x = 0.0,
        }

        let proposed_z = final_pos + Vec3::new(0.0, 0.0, agent.velocity.z * delta_time);
        let (gx, gz) = self.world_to_grid(proposed_z);
        match self.link_to(from, gx, gz) {
            Some(s) => {
                final_pos.z = proposed_z.z;
                on = s;
            }
            None => agent.velocity.z = 0.0,
        }

        final_pos.y = self.surface_y(on, final_pos.x, final_pos.z);
        agent.body(final_pos)
    }
}

/// In place: a detach would change archetype and copy every column.
fn hold_agent(scene: &mut Scene, id: u32) -> Option<NavMeshAgentComponent> {
    scene
        .world
        .nav_agent_mut(id)
        .map(|mut a| std::mem::take(&mut *a))
}

fn put_back(scene: &mut Scene, id: u32, agent: NavMeshAgentComponent) {
    if let Some(mut slot) = scene.world.nav_agent_mut(id) {
        *slot = agent;
    }
}

fn avoidance_input(
    agent: &NavMeshAgentComponent,
    position: Vec3,
    moved_with: Vec3,
    steering: bool,
) -> AvoidanceAgent {
    AvoidanceAgent {
        position: xz(position),
        velocity: xz(moved_with),
        preferred: xz(agent.velocity),
        radius: agent.radius,
        max_speed: agent.speed,
        priority: agent.avoidance_priority,
        solves: steering && agent.avoidance_enabled,
    }
}

fn path_left(agent: &NavMeshAgentComponent, feet: Vec3, next: Vec3) -> f32 {
    let rest = agent.cached_path.get(agent.path_cursor..).unwrap_or(&[]);
    feet.distance(next) + path_length(rest)
}

/// Shedding `a·dt` a tick from `v` covers `v²/(2a) + v·dt/2`; solving that for `v` keeps
/// the step from overshooting the continuous `√(2a·room)`.
pub(super) fn braking_speed(acceleration: f32, room: f32, dt: f32) -> f32 {
    let half_step = 0.5 * acceleration * dt;
    (half_step * half_step + 2.0 * acceleration * room).sqrt() - half_step
}

fn decelerate(agent: &mut NavMeshAgentComponent, delta_time: f32) {
    let v = move_towards(
        xz(agent.velocity),
        Vec2::ZERO,
        agent.acceleration * delta_time,
    );
    agent.velocity = Vec3::new(v.x, 0.0, v.y);
}

/// A negative step holds still.
fn move_towards(from: Vec2, to: Vec2, max_delta: f32) -> Vec2 {
    from.move_towards(to, max_delta.max(0.0))
}
