use crate::core::scene::{ColliderComponent, ColliderShape, Scene};
use glam::{Mat4, Vec3};

#[derive(Copy, Clone, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Evaluates the ray at parameter t
    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Tests intersection of a Ray with an Axis-Aligned Bounding Box (AABB).
/// Returns Some(t) where t is the intersection distance along the ray, or None.
pub fn ray_aabb_intersection(ray: &Ray, min: Vec3, max: Vec3) -> Option<f32> {
    let mut t_min = f32::MIN;
    let mut t_max = f32::MAX;

    // Check X axis
    if ray.direction.x.abs() > f32::EPSILON {
        let tx1 = (min.x - ray.origin.x) / ray.direction.x;
        let tx2 = (max.x - ray.origin.x) / ray.direction.x;
        t_min = t_min.max(tx1.min(tx2));
        t_max = t_max.min(tx1.max(tx2));
    } else if ray.origin.x < min.x || ray.origin.x > max.x {
        return None;
    }

    // Check Y axis
    if ray.direction.y.abs() > f32::EPSILON {
        let ty1 = (min.y - ray.origin.y) / ray.direction.y;
        let ty2 = (max.y - ray.origin.y) / ray.direction.y;
        t_min = t_min.max(ty1.min(ty2));
        t_max = t_max.min(ty1.max(ty2));
    } else if ray.origin.y < min.y || ray.origin.y > max.y {
        return None;
    }

    // Check Z axis
    if ray.direction.z.abs() > f32::EPSILON {
        let tz1 = (min.z - ray.origin.z) / ray.direction.z;
        let tz2 = (max.z - ray.origin.z) / ray.direction.z;
        t_min = t_min.max(tz1.min(tz2));
        t_max = t_max.min(tz1.max(tz2));
    } else if ray.origin.z < min.z || ray.origin.z > max.z {
        return None;
    }

    // If near intersection is further than far, or far is behind ray, no intersection
    if t_min > t_max || t_max < 0.0 {
        return None;
    }

    // If t_min is negative, the ray origin is inside the box, so we return 0.0 (or t_min)
    if t_min < 0.0 {
        Some(0.0)
    } else {
        Some(t_min)
    }
}

/// Casts a ray into the scene and finds the closest intersecting entity with an active collider.
/// Returns Some((entity_id, distance_t))
pub fn cast_ray_in_scene(ray: &Ray, scene: &Scene) -> Option<(u32, f32)> {
    let mut closest_entity = None;
    let mut min_t = f32::MAX;

    for entity in &scene.entities {
        if !entity.active {
            continue;
        }

        // Only test against active colliders (excluding the Player itself to avoid self-hits)
        if let Some(collider) = &entity.collider {
            if !collider.active || entity.name == "Player" {
                continue;
            }

            // Exclude dead entities
            if let Some(health) = &entity.health {
                if health.is_dead {
                    continue;
                }
            }

            // Perform ray-AABB test
            if let Some(t) = ray_aabb_intersection(ray, collider.aabb_min, collider.aabb_max) {
                if t < min_t {
                    min_t = t;
                    closest_entity = Some(entity.id);
                }
            }
        }
    }

    closest_entity.map(|id| (id, min_t))
}

pub struct CollisionContact {
    pub normal: Vec3, // Pointing from B to A (A resolves along normal, B resolves along -normal)
    pub depth: f32,
}

pub fn aabb_overlap(min_a: Vec3, max_a: Vec3, min_b: Vec3, max_b: Vec3) -> bool {
    min_a.x <= max_b.x
        && max_a.x >= min_b.x
        && min_a.y <= max_b.y
        && max_a.y >= min_b.y
        && min_a.z <= max_b.z
        && max_a.z >= min_b.z
}

pub fn test_narrowphase_collision(
    col_a: &ColliderComponent,
    world_mat_a: &Mat4,
    col_b: &ColliderComponent,
    world_mat_b: &Mat4,
) -> Option<CollisionContact> {
    let (scale_a, _, pos_a) = world_mat_a.to_scale_rotation_translation();
    let (scale_b, _, pos_b) = world_mat_b.to_scale_rotation_translation();

    match (&col_a.shape, &col_b.shape) {
        // 1. Box vs Box
        (ColliderShape::Box { size: size_a }, ColliderShape::Box { size: size_b }) => {
            let half_a = *size_a * scale_a * 0.5;
            let half_b = *size_b * scale_b * 0.5;

            let d = pos_a - pos_b;
            let overlap_x = (half_a.x + half_b.x) - d.x.abs();
            let overlap_y = (half_a.y + half_b.y) - d.y.abs();
            let overlap_z = (half_a.z + half_b.z) - d.z.abs();

            if overlap_x > 0.0 && overlap_y > 0.0 && overlap_z > 0.0 {
                if overlap_x < overlap_y && overlap_x < overlap_z {
                    let normal = Vec3::new(if d.x > 0.0 { 1.0 } else { -1.0 }, 0.0, 0.0);
                    Some(CollisionContact {
                        normal,
                        depth: overlap_x,
                    })
                } else if overlap_y < overlap_x && overlap_y < overlap_z {
                    let normal = Vec3::new(0.0, if d.y > 0.0 { 1.0 } else { -1.0 }, 0.0);
                    Some(CollisionContact {
                        normal,
                        depth: overlap_y,
                    })
                } else {
                    let normal = Vec3::new(0.0, 0.0, if d.z > 0.0 { 1.0 } else { -1.0 });
                    Some(CollisionContact {
                        normal,
                        depth: overlap_z,
                    })
                }
            } else {
                None
            }
        }

        // 2. Sphere vs Sphere
        (ColliderShape::Sphere { radius: r_a }, ColliderShape::Sphere { radius: r_b }) => {
            let rad_a = *r_a * scale_a.max_element();
            let rad_b = *r_b * scale_b.max_element();

            let d = pos_a - pos_b;
            let len = d.length();
            let sum_r = rad_a + rad_b;
            if len < sum_r {
                let normal = if len > 0.0001 { d / len } else { Vec3::Y };
                Some(CollisionContact {
                    normal,
                    depth: sum_r - len,
                })
            } else {
                None
            }
        }

        // 3. Sphere vs Box
        (ColliderShape::Sphere { radius: r_s }, ColliderShape::Box { size: size_b }) => {
            let rad_s = *r_s * scale_a.max_element();
            let half_b = *size_b * scale_b * 0.5;

            let d = pos_a - pos_b;
            let closest = pos_b + d.clamp(-half_b, half_b);
            let dist_v = pos_a - closest;
            let len = dist_v.length();

            if len < rad_s {
                if len > 0.0001 {
                    Some(CollisionContact {
                        normal: dist_v / len,
                        depth: rad_s - len,
                    })
                } else {
                    let overlap_x = half_b.x - d.x.abs();
                    let overlap_y = half_b.y - d.y.abs();
                    let overlap_z = half_b.z - d.z.abs();
                    if overlap_x < overlap_y && overlap_x < overlap_z {
                        let normal = Vec3::new(if d.x > 0.0 { 1.0 } else { -1.0 }, 0.0, 0.0);
                        Some(CollisionContact {
                            normal,
                            depth: overlap_x + rad_s,
                        })
                    } else if overlap_y < overlap_x && overlap_y < overlap_z {
                        let normal = Vec3::new(0.0, if d.y > 0.0 { 1.0 } else { -1.0 }, 0.0);
                        Some(CollisionContact {
                            normal,
                            depth: overlap_y + rad_s,
                        })
                    } else {
                        let normal = Vec3::new(0.0, 0.0, if d.z > 0.0 { 1.0 } else { -1.0 });
                        Some(CollisionContact {
                            normal,
                            depth: overlap_z + rad_s,
                        })
                    }
                }
            } else {
                None
            }
        }

        // Box vs Sphere
        (ColliderShape::Box { size: size_a }, ColliderShape::Sphere { radius: r_s }) => {
            let rad_s = *r_s * scale_b.max_element();
            let half_a = *size_a * scale_a * 0.5;

            let d = pos_b - pos_a;
            let closest = pos_a + d.clamp(-half_a, half_a);
            let dist_v = pos_b - closest;
            let len = dist_v.length();

            if len < rad_s {
                if len > 0.0001 {
                    Some(CollisionContact {
                        normal: -dist_v / len,
                        depth: rad_s - len,
                    })
                } else {
                    let overlap_x = half_a.x - d.x.abs();
                    let overlap_y = half_a.y - d.y.abs();
                    let overlap_z = half_a.z - d.z.abs();
                    if overlap_x < overlap_y && overlap_x < overlap_z {
                        let normal = Vec3::new(if d.x > 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0);
                        Some(CollisionContact {
                            normal,
                            depth: overlap_x + rad_s,
                        })
                    } else if overlap_y < overlap_x && overlap_y < overlap_z {
                        let normal = Vec3::new(0.0, if d.y > 0.0 { -1.0 } else { 1.0 }, 0.0);
                        Some(CollisionContact {
                            normal,
                            depth: overlap_y + rad_s,
                        })
                    } else {
                        let normal = Vec3::new(0.0, 0.0, if d.z > 0.0 { -1.0 } else { 1.0 });
                        Some(CollisionContact {
                            normal,
                            depth: overlap_z + rad_s,
                        })
                    }
                }
            } else {
                None
            }
        }

        // 4. Cylinder vs Cylinder
        (
            ColliderShape::Cylinder {
                radius: r_a,
                height: h_a,
            },
            ColliderShape::Cylinder {
                radius: r_b,
                height: h_b,
            },
        ) => {
            let rad_a = *r_a * scale_a.x;
            let rad_b = *r_b * scale_b.x;
            let half_h_a = *h_a * scale_a.y * 0.5;
            let half_h_b = *h_b * scale_b.y * 0.5;

            let d_xz = Vec3::new(pos_a.x - pos_b.x, 0.0, pos_a.z - pos_b.z);
            let len_xz = d_xz.length();
            let sum_r = rad_a + rad_b;

            if len_xz >= sum_r {
                return None;
            }

            let min_y_a = pos_a.y - half_h_a;
            let max_y_a = pos_a.y + half_h_a;
            let min_y_b = pos_b.y - half_h_b;
            let max_y_b = pos_b.y + half_h_b;

            if max_y_a <= min_y_b || min_y_a >= max_y_b {
                return None;
            }

            let depth_xz = sum_r - len_xz;
            let (depth_y, n_y) = if pos_a.y > pos_b.y {
                (max_y_b - min_y_a, Vec3::Y)
            } else {
                (max_y_a - min_y_b, -Vec3::Y)
            };

            if depth_xz < depth_y {
                let normal = if len_xz > 0.0001 {
                    d_xz / len_xz
                } else {
                    Vec3::X
                };
                Some(CollisionContact {
                    normal,
                    depth: depth_xz,
                })
            } else {
                Some(CollisionContact {
                    normal: n_y,
                    depth: depth_y,
                })
            }
        }

        // 5. Cylinder vs Box
        (
            ColliderShape::Cylinder {
                radius: r_c,
                height: h_c,
            },
            ColliderShape::Box { size: size_b },
        ) => {
            let rad_c = *r_c * scale_a.x;
            let half_h_c = *h_c * scale_a.y * 0.5;
            let half_b = *size_b * scale_b * 0.5;

            let closest_x = pos_a.x.clamp(pos_b.x - half_b.x, pos_b.x + half_b.x);
            let closest_z = pos_a.z.clamp(pos_b.z - half_b.z, pos_b.z + half_b.z);

            let dx = pos_a.x - closest_x;
            let dz = pos_a.z - closest_z;
            let len_xz = (dx * dx + dz * dz).sqrt();

            if len_xz >= rad_c && (closest_x != pos_a.x || closest_z != pos_a.z) {
                return None;
            }

            let min_y_c = pos_a.y - half_h_c;
            let max_y_c = pos_a.y + half_h_c;
            let min_y_b = pos_b.y - half_b.y;
            let max_y_b = pos_b.y + half_b.y;

            if max_y_c <= min_y_b || min_y_c >= max_y_b {
                return None;
            }

            let depth_xz = if len_xz > 0.0001 {
                rad_c - len_xz
            } else {
                rad_c
            };
            let (depth_y, n_y) = if pos_a.y > pos_b.y {
                (max_y_b - min_y_c, Vec3::Y)
            } else {
                (max_y_c - min_y_b, -Vec3::Y)
            };

            if depth_xz < depth_y {
                let normal = if len_xz > 0.0001 {
                    Vec3::new(dx, 0.0, dz).normalize()
                } else {
                    Vec3::X
                };
                Some(CollisionContact {
                    normal,
                    depth: depth_xz,
                })
            } else {
                Some(CollisionContact {
                    normal: n_y,
                    depth: depth_y,
                })
            }
        }

        // Box vs Cylinder
        (
            ColliderShape::Box { size: size_a },
            ColliderShape::Cylinder {
                radius: r_c,
                height: h_c,
            },
        ) => {
            let rad_c = *r_c * scale_b.x;
            let half_h_c = *h_c * scale_b.y * 0.5;
            let half_a = *size_a * scale_a * 0.5;

            let closest_x = pos_b.x.clamp(pos_a.x - half_a.x, pos_a.x + half_a.x);
            let closest_z = pos_b.z.clamp(pos_a.z - half_a.z, pos_a.z + half_a.z);

            let dx = pos_b.x - closest_x;
            let dz = pos_b.z - closest_z;
            let len_xz = (dx * dx + dz * dz).sqrt();

            if len_xz >= rad_c && (closest_x != pos_b.x || closest_z != pos_b.z) {
                return None;
            }

            let min_y_c = pos_b.y - half_h_c;
            let max_y_c = pos_b.y + half_h_c;
            let min_y_a = pos_a.y - half_a.y;
            let max_y_a = pos_a.y + half_a.y;

            if max_y_a <= min_y_c || min_y_a >= max_y_c {
                return None;
            }

            let depth_xz = if len_xz > 0.0001 {
                rad_c - len_xz
            } else {
                rad_c
            };
            let (depth_y, n_y) = if pos_b.y > pos_a.y {
                (max_y_a - min_y_c, -Vec3::Y)
            } else {
                (max_y_c - min_y_a, Vec3::Y)
            };

            if depth_xz < depth_y {
                let normal = if len_xz > 0.0001 {
                    -Vec3::new(dx, 0.0, dz).normalize()
                } else {
                    -Vec3::X
                };
                Some(CollisionContact {
                    normal,
                    depth: depth_xz,
                })
            } else {
                Some(CollisionContact {
                    normal: n_y,
                    depth: depth_y,
                })
            }
        }

        // 6. Sphere vs Cylinder
        (
            ColliderShape::Sphere { radius: r_s },
            ColliderShape::Cylinder {
                radius: r_c,
                height: h_c,
            },
        ) => {
            let rad_s = *r_s * scale_a.max_element();
            let rad_c = *r_c * scale_b.x;
            let half_h_c = *h_c * scale_b.y * 0.5;

            let d_xz = Vec3::new(pos_a.x - pos_b.x, 0.0, pos_a.z - pos_b.z);
            let len_xz = d_xz.length();

            let closest_y = pos_a.y.clamp(pos_b.y - half_h_c, pos_b.y + half_h_c);
            let dir_xz = if len_xz > 0.0001 {
                d_xz / len_xz
            } else {
                Vec3::X
            };
            let cx = pos_b.x + dir_xz.x * rad_c.min(len_xz);
            let cz = pos_b.z + dir_xz.z * rad_c.min(len_xz);

            let closest = Vec3::new(cx, closest_y, cz);
            let dist_v = pos_a - closest;
            let len = dist_v.length();

            if len < rad_s {
                let normal = if len > 0.0001 { dist_v / len } else { Vec3::Y };
                Some(CollisionContact {
                    normal,
                    depth: rad_s - len,
                })
            } else {
                None
            }
        }

        // Cylinder vs Sphere
        (
            ColliderShape::Cylinder {
                radius: r_c,
                height: h_c,
            },
            ColliderShape::Sphere { radius: r_s },
        ) => {
            let rad_s = *r_s * scale_b.max_element();
            let rad_c = *r_c * scale_a.x;
            let half_h_c = *h_c * scale_a.y * 0.5;

            let d_xz = Vec3::new(pos_b.x - pos_a.x, 0.0, pos_b.z - pos_a.z);
            let len_xz = d_xz.length();

            let closest_y = pos_b.y.clamp(pos_a.y - half_h_c, pos_a.y + half_h_c);
            let dir_xz = if len_xz > 0.0001 {
                d_xz / len_xz
            } else {
                Vec3::X
            };
            let cx = pos_a.x + dir_xz.x * rad_c.min(len_xz);
            let cz = pos_a.z + dir_xz.z * rad_c.min(len_xz);

            let closest = Vec3::new(cx, closest_y, cz);
            let dist_v = pos_b - closest;
            let len = dist_v.length();

            if len < rad_s {
                let normal = if len > 0.0001 {
                    -dist_v / len
                } else {
                    -Vec3::Y
                };
                Some(CollisionContact {
                    normal,
                    depth: rad_s - len,
                })
            } else {
                None
            }
        }
    }
}

pub fn tick_physics(scene: &mut Scene, delta_time: f32) -> Vec<(u32, u32)> {
    let mut trigger_events = Vec::new();

    // A. Apply Gravity & Update Positions for dynamic RigidBodies
    let gravity = Vec3::new(0.0, -9.81, 0.0);
    for entity in &mut scene.entities {
        if !entity.active {
            continue;
        }

        if let Some(rb) = &mut entity.rigidbody {
            if rb.active && !rb.is_kinematic {
                if rb.use_gravity {
                    rb.velocity += gravity * delta_time;
                }

                entity.transform.position += rb.velocity * delta_time;
            }
        }
    }

    // B. Refresh world-space collider bounds
    scene.update_all_colliders();

    // C. Collision Resolution (Iterative solver passes)
    let num_entities = scene.entities.len();
    let num_passes = 2;

    for _pass in 0..num_passes {
        for i in 0..num_entities {
            for j in (i + 1)..num_entities {
                let (id_a, id_b) = {
                    let ent_a = &scene.entities[i];
                    let ent_b = &scene.entities[j];
                    if !ent_a.active || !ent_b.active {
                        continue;
                    }
                    (ent_a.id, ent_b.id)
                };

                // 1. Broadphase AABB Check
                let (col_a_active, col_a_trigger, aabb_min_a, aabb_max_a) = {
                    let ent_a = &scene.entities[i];
                    match &ent_a.collider {
                        Some(c) => (c.active, c.is_trigger, c.aabb_min, c.aabb_max),
                        None => (false, false, Vec3::ZERO, Vec3::ZERO),
                    }
                };

                let (col_b_active, col_b_trigger, aabb_min_b, aabb_max_b) = {
                    let ent_b = &scene.entities[j];
                    match &ent_b.collider {
                        Some(c) => (c.active, c.is_trigger, c.aabb_min, c.aabb_max),
                        None => (false, false, Vec3::ZERO, Vec3::ZERO),
                    }
                };

                if !col_a_active || !col_b_active {
                    continue;
                }

                if !aabb_overlap(aabb_min_a, aabb_max_a, aabb_min_b, aabb_max_b) {
                    continue;
                }

                // 2. Narrowphase & Resolution
                let world_matrix_a = scene.compute_world_matrix(id_a);
                let world_matrix_b = scene.compute_world_matrix(id_b);

                let contact = {
                    let ent_a = &scene.entities[i];
                    let ent_b = &scene.entities[j];
                    test_narrowphase_collision(
                        ent_a.collider.as_ref().unwrap(),
                        &world_matrix_a,
                        ent_b.collider.as_ref().unwrap(),
                        &world_matrix_b,
                    )
                };

                if let Some(contact) = contact {
                    if col_a_trigger || col_b_trigger {
                        trigger_events.push((id_a, id_b));
                        continue;
                    }

                    let type_a = {
                        let ent_a = &scene.entities[i];
                        if ent_a.is_static {
                            0
                        } else if ent_a.rigidbody.as_ref().map_or(true, |r| r.is_kinematic) {
                            1
                        } else {
                            2
                        }
                    };
                    let type_b = {
                        let ent_b = &scene.entities[j];
                        if ent_b.is_static {
                            0
                        } else if ent_b.rigidbody.as_ref().map_or(true, |r| r.is_kinematic) {
                            1
                        } else {
                            2
                        }
                    };

                    match (type_a, type_b) {
                        (0, 0) => {}

                        (0, 1) | (0, 2) => {
                            let push = -contact.normal * contact.depth;
                            if let Some(ent_b_mut) = scene.get_entity_mut(id_b) {
                                ent_b_mut.transform.position += push;
                                if let Some(rb_b) = &mut ent_b_mut.rigidbody {
                                    let normal_vel = rb_b.velocity.dot(contact.normal);
                                    if normal_vel > 0.0 {
                                        rb_b.velocity -= contact.normal * normal_vel;
                                    }
                                }
                            }
                            scene.update_entity_collider(id_b);
                        }

                        (1, 0) | (2, 0) => {
                            let push = contact.normal * contact.depth;
                            if let Some(ent_a_mut) = scene.get_entity_mut(id_a) {
                                ent_a_mut.transform.position += push;
                                if let Some(rb_a) = &mut ent_a_mut.rigidbody {
                                    let normal_vel = rb_a.velocity.dot(contact.normal);
                                    if normal_vel < 0.0 {
                                        rb_a.velocity -= contact.normal * normal_vel;
                                    }
                                }
                            }
                            scene.update_entity_collider(id_a);
                        }

                        (1, 1) => {
                            let push_a = contact.normal * (contact.depth * 0.5);
                            let push_b = -contact.normal * (contact.depth * 0.5);

                            if let Some(ent_a_mut) = scene.get_entity_mut(id_a) {
                                ent_a_mut.transform.position += push_a;
                            }
                            scene.update_entity_collider(id_a);

                            if let Some(ent_b_mut) = scene.get_entity_mut(id_b) {
                                ent_b_mut.transform.position += push_b;
                            }
                            scene.update_entity_collider(id_b);
                        }

                        (1, 2) => {
                            let push = -contact.normal * contact.depth;
                            if let Some(ent_b_mut) = scene.get_entity_mut(id_b) {
                                ent_b_mut.transform.position += push;
                                if let Some(rb_b) = &mut ent_b_mut.rigidbody {
                                    let normal_vel = rb_b.velocity.dot(contact.normal);
                                    if normal_vel > 0.0 {
                                        rb_b.velocity -= contact.normal * normal_vel;
                                    }
                                }
                            }
                            scene.update_entity_collider(id_b);
                        }

                        (2, 1) => {
                            let push = contact.normal * contact.depth;
                            if let Some(ent_a_mut) = scene.get_entity_mut(id_a) {
                                ent_a_mut.transform.position += push;
                                if let Some(rb_a) = &mut ent_a_mut.rigidbody {
                                    let normal_vel = rb_a.velocity.dot(contact.normal);
                                    if normal_vel < 0.0 {
                                        rb_a.velocity -= contact.normal * normal_vel;
                                    }
                                }
                            }
                            scene.update_entity_collider(id_a);
                        }

                        (2, 2) => {
                            let push_a = contact.normal * (contact.depth * 0.5);
                            let push_b = -contact.normal * (contact.depth * 0.5);

                            if let Some(ent_a_mut) = scene.get_entity_mut(id_a) {
                                ent_a_mut.transform.position += push_a;
                                if let Some(rb_a) = &mut ent_a_mut.rigidbody {
                                    let normal_vel = rb_a.velocity.dot(contact.normal);
                                    if normal_vel < 0.0 {
                                        rb_a.velocity -= contact.normal * normal_vel;
                                    }
                                }
                            }
                            scene.update_entity_collider(id_a);

                            if let Some(ent_b_mut) = scene.get_entity_mut(id_b) {
                                ent_b_mut.transform.position += push_b;
                                if let Some(rb_b) = &mut ent_b_mut.rigidbody {
                                    let normal_vel = rb_b.velocity.dot(contact.normal);
                                    if normal_vel > 0.0 {
                                        rb_b.velocity -= contact.normal * normal_vel;
                                    }
                                }
                            }
                            scene.update_entity_collider(id_b);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    trigger_events
}
