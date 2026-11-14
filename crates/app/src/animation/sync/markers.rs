//! crates/app/src/animation/sync/markers.rs — the phase arithmetic of sync markers
//! (#1896): where between which pair of markers a clip is, and the time that
//! puts another clip at the same place. Pure functions of (markers, length,
//! time); a marker list is sorted by time, as the graph's load leaves it.

use rusty_asset::animation_graph::SyncMarker;
use rusty_components::wrap_delta;

/// Where in its cycle a clip is: its normalized phase, and with markers the
/// pair it is between and the fraction of the way from the first to the
/// second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncPos<'a> {
    pub phase: f32,
    pub pair: Option<(&'a str, &'a str, f32)>,
}

/// `t` seconds into a clip of `duration` with `markers`, as a [`SyncPos`]. A
/// looping clip's pair wraps round its seam; a non-looping one before its
/// first marker or past its last has none.
pub fn position(markers: &[SyncMarker], duration: f32, t: f32, looping: bool) -> SyncPos<'_> {
    let phase = if duration > 0.0 { t / duration } else { 0.0 };
    SyncPos {
        phase,
        pair: pair_at(markers, duration, t, looping),
    }
}

fn pair_at(
    markers: &[SyncMarker],
    duration: f32,
    t: f32,
    looping: bool,
) -> Option<(&str, &str, f32)> {
    if markers.is_empty() || duration <= 0.0 {
        return None;
    }
    // The last marker at or before `t` (the later of a tie), and the first
    // after it: a list sorted by time, as the load leaves it.
    let prev = markers.iter().rposition(|m| m.time <= t);
    let next = markers.iter().position(|m| m.time > t);
    let (tp, ip) = match prev {
        Some(i) => (markers[i].time, i),
        None if looping => (
            markers[markers.len() - 1].time - duration,
            markers.len() - 1,
        ),
        None => return None,
    };
    let (tn, jn) = match next {
        Some(j) => (markers[j].time, j),
        None if looping => (markers[0].time + duration, 0),
        None => return None,
    };
    let span = tn - tp;
    let frac = if span > 0.0 {
        ((t - tp) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((&markers[ip].name, &markers[jn].name, frac))
}

/// The time in a clip of `duration` with `markers` at `pos`: the same fraction
/// of the way between its own pair of the same names, the pair nearest `near`
/// when it has several, or the normalized phase without one.
pub fn place(markers: &[SyncMarker], duration: f32, pos: SyncPos, near: f32, looping: bool) -> f32 {
    let fallback = pos.phase * duration;
    let Some((first, second, frac)) = pos.pair.filter(|_| duration > 0.0) else {
        return fallback;
    };
    let distance = |t: f32| match looping {
        true => wrap_delta(t - near, duration).abs(),
        false => (t - near).abs(),
    };
    let mut best: Option<(f32, f32)> = None;
    for (i, marker) in markers.iter().enumerate().filter(|(_, m)| m.name == first) {
        // The marker after this one, round the seam when looping.
        let after = match markers[i + 1..].iter().find(|m| m.time > marker.time) {
            Some(m) => Some((m, m.time)),
            None if looping => markers.first().map(|m| (m, m.time + duration)),
            None => None,
        };
        let Some((_, tn)) = after.filter(|(m, _)| m.name == second) else {
            continue;
        };
        let t = marker.time + frac * (tn - marker.time);
        let t = if looping { t.rem_euclid(duration) } else { t };
        if best.is_none_or(|(d, _)| distance(t) < d) {
            best = Some((distance(t), t));
        }
    }
    best.map_or(fallback, |(_, t)| t)
}
