//! Hashed straight from the world's fields, never a serialized snapshot: serializing a
//! Dust2-sized scene at every digest would hitch a playtest.

use rusty_app::GameWorld;

/// Ticks between two digests: two seconds at 60 Hz.
pub const EVERY: u64 = 120;

pub fn due(t: u64) -> bool {
    (t + 1).is_multiple_of(EVERY)
}

/// Gathered into one buffer for one xxh3 pass, not hashed byte by byte at opt-level 0.
#[derive(Default)]
struct Fields(Vec<u8>);

impl Fields {
    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn u64(&mut self, x: u64) {
        self.bytes(&x.to_le_bytes());
    }

    fn floats(&mut self, xs: &[f32]) {
        for x in xs {
            self.bytes(&x.to_bits().to_le_bytes());
        }
    }
}

pub fn of(world: &GameWorld) -> String {
    let mut h = Fields::default();
    let scene = world.scene().borrow();
    let ecs = &scene.world;
    for &id in ecs.ids() {
        h.u64(u64::from(id));
        if let Some(name) = ecs.name(id) {
            h.bytes(name.as_bytes());
        }
        // lint: allow(active-self) the digest keeps the flag itself, as a save would
        h.bytes(&[u8::from(ecs.is_active_self(id))]);
        if let Some(t) = ecs.transform(id) {
            h.floats(&t.position.to_array());
            h.floats(&t.rotation.to_array());
            h.floats(&t.scale.to_array());
        }
        if let Some(body) = ecs.rigidbody(id) {
            h.floats(&body.velocity.to_array());
            h.floats(&body.angular_velocity.to_array());
        }
    }
    // A replay that shifts on another tick, or by other steps, diverges here.
    let origin = scene.loaded.origin.steps().to_array();
    h.bytes(&origin.map(i32::to_le_bytes).concat());
    h.u64(world.play_frame());
    h.u64(world.resources.time.borrow().time.to_bits());
    h.u64(world.script_manager().random_state());
    format!("{:016x}", rusty_core::hash::content(&h.0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_fields_are_laid_out_little_endian() {
        let mut h = super::Fields::default();
        h.u64(1);
        h.floats(&[1.0]);
        assert_eq!(h.0, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x80, 0x3f]);
    }
}
