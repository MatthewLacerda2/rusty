//! Shared setup for the automation tests.

use std::collections::BTreeMap;

use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Patch, Slope};
use scorsese_zimmer::song::{
    Automation, Easing, InlineOnly, Param, PatchRef, Pattern, Point, Song, Track,
};
use scorsese_zimmer::{SAMPLE_RATE, render_song};

use crate::common::songs::{note, played};
use crate::common::{channel, saw_patch};

pub(crate) const BEATS: f32 = 8.0;

pub(crate) fn held(gain: f32) -> Song {
    voiced(saw_patch(), gain)
}

pub(crate) fn voiced(patch: Patch, gain: f32) -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "a".to_owned(),
        Pattern {
            beats: BEATS,
            notes: played(vec![note("pad", "A3", 0.0, BEATS)]),
        },
    );
    Song {
        bpm: 120.0,
        tempo: vec![],
        seed: 3,
        key: None,
        tracks: vec![Track {
            name: "pad".to_owned(),
            patch: PatchRef::Inline(Box::new(patch)),
            gain,
            pan: 0.0,
            fx: vec![],
            send: 1.0,
            push: 0.0,
            swing: None,
            humanize: None,
        }],
        patterns,
        arrangement: vec!["a".into()],
        swing: 0.0,
        humanize: None,
        fx: vec![],
        automation: vec![],
        fit: None,
        anchors: Vec::new(),
        fade: None,
        tail: None,
    }
}

pub(crate) fn filtered(cutoff: f32) -> Patch {
    Patch {
        filter: Some(Filter {
            kind: FilterKind::Lowpass,
            slope: Slope::Db12,
            cutoff,
            resonance: 0.0,
            env_octaves: 0.0,
            vel_octaves: 0.0,
            key_track: 0.0,
            adsr: Adsr::default(),
        }),
        ..saw_patch()
    }
}

pub(crate) fn curve(param: Param, points: Vec<Point>) -> Automation {
    Automation {
        track: "pad".to_owned(),
        param,
        points,
    }
}

pub(crate) fn at(beat: f32, value: f32) -> Point {
    Point {
        beat,
        value,
        easing: Easing::Linear,
    }
}

pub(crate) fn eased(beat: f32, value: f32, easing: Easing) -> Point {
    Point {
        beat,
        value,
        easing,
    }
}

pub(crate) fn ramp(param: Param, from: f32, to: f32) -> Automation {
    curve(param, vec![at(0.0, from), at(BEATS, to)])
}

pub(crate) fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

pub(crate) fn sides(song: &Song) -> (Vec<f32>, Vec<f32>) {
    let mix = render_song(song, &InlineOnly).expect("the song renders");
    (channel(&mix, 0), channel(&mix, 1))
}

pub(crate) fn frame(beat: f32) -> usize {
    (beat * 0.5 * SAMPLE_RATE as f32).round() as usize
}

pub(crate) fn rms(buf: &[f32]) -> f32 {
    let sum: f32 = buf.iter().map(|sample| sample * sample).sum();
    (sum / buf.len() as f32).sqrt()
}
