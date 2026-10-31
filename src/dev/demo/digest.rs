//! The state digest a demo stores every [`EVERY`] ticks and when Play ends, and a
//! replay compares (#1581): what makes "bit for bit" a check, not a promise.
//!
//! It hashes the fields straight from the world, never a serialized snapshot, so
//! a Dust2-sized scene does not hitch every two seconds of a playtest: every
//! entity's id, name, own active flag and transform, every rigidbody's linear and
//! angular velocity, the play frame, `Time.time`, and where the `Random` stream
//! stands, so a draw the world shows nothing of yet still counts. It does **not**
//! cover script state (a Lua table's fields): a script that diverges shows up
//! once it moves something.

use crate::app::GameWorld;

/// Ticks between two digests: two seconds at 60 Hz.
pub const EVERY: u64 = 120;

/// Whether the tick `t` (from the one that entered Play) ends with a digest.
pub fn due(t: u64) -> bool {
    (t + 1).is_multiple_of(EVERY)
}

/// The fields, gathered into one buffer for [`crate::core::hash::content`]: one
/// xxh3 pass, not a byte-at-a-time hash run at opt-level 0 (#1326).
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

/// The world's state as one hex string.
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
    h.u64(world.play_frame());
    h.u64(world.resources.time.borrow().time.to_bits());
    h.u64(world.script_manager().random_state());
    format!("{:016x}", crate::core::hash::content(&h.0))
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
