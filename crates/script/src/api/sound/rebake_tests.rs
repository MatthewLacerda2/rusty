use super::fixture::{lua_with_sound, tmp, HELD_PATCH};

#[test]
fn the_clip_cache_follows_bakes_at_one_path() {
    use rusty_test_support::rebakes::{follows_rebakes, Bake};
    let lua = lua_with_sound();
    let mut cache = rusty_audio::device::decode::ClipCache::new();
    let seconds = |which| if which == Bake::A { 0.15 } else { 0.6 };
    let path = tmp("rusty_sound_rebake.wav");
    follows_rebakes(
        &path,
        |p, which| {
            let duration = seconds(which);
            let call =
                format!(r#"Sound.Bake({HELD_PATCH}, "C2", "{p}", {{ duration = {duration} }})"#);
            lua.load(call).exec().expect("the bake runs");
        },
        |p| cache.get_or_decode(p).map(|clip| clip.frames()),
        |frames, which| {
            let want = seconds(which) * rusty_zimmer::SAMPLE_RATE as f32;
            frames.is_some_and(|n| (n as f32 - want).abs() < want * 0.5)
        },
    );
    std::fs::remove_file(path).ok();
}
