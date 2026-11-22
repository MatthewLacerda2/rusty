use rusty_asset::animation_graph::SyncMarker;
use rusty_components::wrap_delta;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncPos<'a> {
    pub phase: f32,
    pub pair: Option<(&'a str, &'a str, f32)>,
}

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
