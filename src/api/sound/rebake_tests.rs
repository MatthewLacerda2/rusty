//! A `Sound` re-bake reaches the clip cache (#1208): the cache the device backend
//! keeps for the whole session plays what was baked last at a path.

use super::fixture::{lua_with_sound, tmp, IMPACT_PATCH};

/// The shared cache contract (#1207, #1208): one clip cache, as the device backend
/// keeps for the session, plays a path baked after it was first played, then the
/// re-bake at that path rather than the clip it decoded first.
#[test]
fn the_clip_cache_follows_bakes_at_one_path() {
    use crate::core::fs::written::contract::{follows_rebakes, Bake};
    let lua = lua_with_sound();
    let mut cache = crate::audio::device::decode::ClipCache::new();
    let seconds = |which| if which == Bake::A { 0.15 } else { 0.6 };
    let path = tmp("rusty_sound_rebake.wav");
    follows_rebakes(
        &path,
        |p, which| {
            let duration = seconds(which);
            let call =
                format!(r#"Sound.Bake({IMPACT_PATCH}, "C2", "{p}", {{ duration = {duration} }})"#);
            lua.load(call).exec().expect("the bake runs");
        },
        |p| cache.get_or_decode(p).map(|clip| clip.frames()),
        |frames, which| {
            let want = seconds(which) * zimmer::SAMPLE_RATE as f32;
            frames.is_some_and(|n| (n as f32 - want).abs() < want * 0.5)
        },
    );
    std::fs::remove_file(path).ok();
}
