//! The cost of the pair and of `ForEach`: a round trip over 1,000 particles
//! (read, change each velocity, write back), and `ForEach` running the doc's attractor, the figure the 1 ms bar is held to
//! in the build games are played on.

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "the measurement times the calls with the wall clock; the sim never reads it"
)]

use std::time::Instant;

use mlua::{Function, Lua};
use rusty::script::api::bind;

use super::{emitter, scene_with};

const PARTICLES: u32 = 1000;
const ROUNDS: usize = 200;

/// The median milliseconds of `ROUNDS` calls of `round` (one round trip each).
fn median_ms(round: &Function) -> f64 {
    let mut ms: Vec<f64> = (0..ROUNDS)
        .map(|_| {
            let start = Instant::now();
            round.call::<()>(()).unwrap();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    ms.sort_by(f64::total_cmp);
    ms[ROUNDS / 2]
}

/// `cargo test --release --features dev --test integration -- --ignored --nocapture particles_round_trip`
#[test]
// lint: allow(skip) a timing measurement, meaningful only in release; run by hand
#[ignore = "measurement for the PR record; run in release"]
fn measure_particles_round_trip() {
    let mut cfg = emitter(PARTICLES);
    cfg.max_particles = PARTICLES;
    let (scene, id) = scene_with(cfg);
    let lua = Lua::new();
    rusty::script::api::particle::register(&lua, bind(&lua, &scene)).unwrap();
    let round = |code: &str| -> Function { lua.load(code).eval().unwrap() };
    let pair = round(&format!(
        "return function()
           local ps = Particles.GetParticles({id})
           for _, p in ipairs(ps) do p.vy = p.vy + 0.01 end
           Particles.SetParticles({id}, ps)
         end"
    ));
    let for_each = round(&format!(
        "local function rule(p) p.vy = p.vy + 0.01 end
         return function() Particles.ForEach({id}, rule) end"
    ));
    // The doc's attractor: six reads and three writes per particle.
    let attract = round(&format!(
        "local function rule(p)
           p.vx = p.vx + (0 - p.x) * 0.01
           p.vy = p.vy + (2 - p.y) * 0.01
           p.vz = p.vz + (0 - p.z) * 0.01
         end
         return function() Particles.ForEach({id}, rule) end"
    ));
    let one = round(&format!(
        "local VY = {{ 'vy' }}
         return function()
           local a = Particles.GetArrays({id}, VY)
           local vy = a.vy
           for i = 1, #vy do vy[i] = vy[i] + 0.01 end
           Particles.SetArrays({id}, a)
         end"
    ));
    let arrays = round(&format!(
        "local FIELDS = {{ 'x', 'y', 'z', 'vx', 'vy', 'vz' }}
         return function()
           local a = Particles.GetArrays({id}, FIELDS)
           local x, y, z, vx, vy, vz = a.x, a.y, a.z, a.vx, a.vy, a.vz
           for i = 1, #x do
             vx[i] = vx[i] + (0 - x[i]) * 0.01
             vy[i] = vy[i] + (2 - y[i]) * 0.01
             vz[i] = vz[i] + (0 - z[i]) * 0.01
           end
           a.x, a.y, a.z = nil, nil, nil
           Particles.SetArrays({id}, a)
         end"
    ));
    let live = || scene.borrow().world.particles(id).unwrap().live_count();
    assert_eq!(live(), PARTICLES as usize);
    let (pair_ms, for_each_ms) = (median_ms(&pair), median_ms(&for_each));
    let (attract_ms, arrays_ms) = (median_ms(&attract), median_ms(&arrays));
    let one_ms = median_ms(&one);
    assert_eq!(
        live(),
        PARTICLES as usize,
        "a round trip keeps every particle"
    );
    eprintln!("{PARTICLES} particles, median of {ROUNDS} round trips:");
    eprintln!("  GetParticles + SetParticles: {pair_ms:.3} ms");
    eprintln!("  ForEach:                     {for_each_ms:.3} ms");
    eprintln!("  GetArrays + SetArrays:       {one_ms:.3} ms");
    eprintln!("  ForEach, the attractor:      {attract_ms:.3} ms");
    eprintln!("  GetArrays, the attractor:    {arrays_ms:.3} ms");
}
