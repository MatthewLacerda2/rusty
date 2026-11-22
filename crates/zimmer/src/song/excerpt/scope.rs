//! An excerpt resolved against the song it excerpts, and the guard that makes it exact.

use super::{Excerpt, Song};
use crate::core::RATE;
use crate::error::SynthError;
use crate::fx;
use crate::song::clock::Clock;
use crate::song::shape::SEAM;
use crate::song::timing::Tail;

/// Frames of slack for the limiter's true-peak reconstruction, which reads `TAPS` (4) frames
/// either side: generous, since one short breaks exactness.
const RECONSTRUCTION: usize = 64;

pub(crate) struct Scope {
    from: usize,
    to: Option<usize>,
    until: Option<usize>,
    heard: Vec<bool>,
}

impl Scope {
    pub(crate) fn of(song: &Song, excerpt: &Excerpt, clock: &Clock) -> Result<Self, SynthError> {
        for name in &excerpt.only {
            if !song.tracks.iter().any(|track| &track.name == name) {
                return Err(SynthError::UnknownSoloTrack {
                    track: name.clone(),
                });
            }
        }
        let heard: Vec<bool> = song
            .tracks
            .iter()
            .map(|track| excerpt.only.is_empty() || excerpt.only.contains(&track.name))
            .collect();
        let (from, to) = excerpt
            .window
            .map_or((0, None), |window| window.frames(clock));
        // A wrapped song's start sums its end, so under `wrap` no note is late enough to skip.
        let wraps = song.tail() == Tail::Wrap;
        Ok(Self {
            from,
            to,
            until: to.filter(|_| !wraps).map(|to| to + guard(song)),
            heard,
        })
    }

    pub(crate) fn heard(&self, track: usize) -> bool {
        self.heard.get(track).copied().unwrap_or(true)
    }

    pub(crate) fn heard_count(&self) -> usize {
        self.heard.iter().filter(|heard| **heard).count()
    }

    pub(crate) fn reaches(&self, at: usize) -> bool {
        self.until.is_none_or(|until| at <= until)
    }

    pub(crate) fn keep(&self, frames: usize) -> (usize, usize) {
        let from = self.from.min(frames);
        (from, self.to.unwrap_or(frames).clamp(from, frames))
    }

    pub(crate) fn opens_at_seconds(&self) -> f64 {
        self.from as f64 / f64::from(RATE)
    }
}

fn guard(song: &Song) -> usize {
    let track = song
        .tracks
        .iter()
        .map(|track| fx::lookahead_seconds(&track.fx))
        .fold(0.0, f32::max);
    let seconds = track + fx::lookahead_seconds(&song.fx) + fx::limiter::LOOKAHEAD + SEAM;
    (seconds * RATE).ceil().max(0.0) as usize + RECONSTRUCTION
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::patch::Fx;
    use crate::song::{PatchRef, Track};
    use crate::song::{Span, Window};

    fn two_tracks() -> Song {
        let track = |name: &str| Track {
            name: name.to_owned(),
            patch: PatchRef::Named("nowhere".to_owned()),
            gain: 1.0,
            pan: 0.0,
            fx: Vec::new(),
            send: 1.0,
            push: 0.0,
            swing: None,
            humanize: None,
        };
        Song {
            bpm: 120.0,
            tempo: vec![],
            seed: 0,
            key: None,
            tracks: vec![track("bass"), track("pad")],
            patterns: BTreeMap::new(),
            arrangement: Vec::new(),
            swing: 0.0,
            humanize: None,
            fx: Vec::new(),
            automation: Vec::new(),
            fit: None,
            anchors: Vec::new(),
            fade: None,
            tail: None,
        }
    }

    fn scope(excerpt: &Excerpt) -> Scope {
        Scope::of(&two_tracks(), excerpt, &Clock::at(120.0)).expect("the excerpt resolves")
    }

    #[test]
    fn a_note_past_the_window_and_its_guard_is_not_rendered() {
        let scope = scope(&Excerpt::of(Window::seconds(
            Span::new(0.0, Some(1.0)).unwrap(),
        )));
        let end = RATE as usize;
        assert!(
            scope.reaches(end),
            "a note at the window's edge still plays"
        );
        assert!(
            scope.reaches(end + RECONSTRUCTION),
            "the guard reaches past the edge"
        );
        assert!(
            !scope.reaches(end + RATE as usize),
            "a note a second late is still being rendered"
        );
    }

    #[test]
    fn an_open_window_never_calls_a_note_late() {
        let scope = scope(&Excerpt::of(Window::beats(Span::new(4.0, None).unwrap())));
        assert!(scope.reaches(usize::MAX));
        let frames = RATE as usize * 5;
        assert_eq!(scope.keep(frames), (RATE as usize * 2, frames));
    }

    #[test]
    fn a_solo_hears_the_tracks_it_names_and_no_others() {
        let scope = scope(&Excerpt::only(vec!["pad".to_owned()]));
        assert!(!scope.heard(0), "the bass was not asked for");
        assert!(scope.heard(1), "the pad was");
        assert_eq!(scope.heard_count(), 1);
        assert_eq!(scope.keep(1_000), (0, 1_000), "a solo keeps every sample");
    }

    #[test]
    fn a_whole_render_hears_everything() {
        let scope = scope(&Excerpt::default());
        assert_eq!(scope.heard_count(), 2);
        assert_eq!(scope.opens_at_seconds(), 0.0);
        assert!(Excerpt::default().is_whole());
    }

    #[test]
    fn asking_for_anything_less_is_not_the_whole_piece() {
        let window = Window::beats(Span::new(0.0, Some(8.0)).expect("a legal span"));
        assert!(!Excerpt::of(window).is_whole(), "a window is not the whole");
        assert!(
            !Excerpt::only(vec!["pad".to_owned()]).is_whole(),
            "and neither is a solo"
        );
    }

    #[test]
    fn a_window_opens_where_it_says_it_does() {
        let window = Window::beats(Span::new(4.0, Some(8.0)).expect("a legal span"));
        let scope = scope(&Excerpt::of(window));
        assert!(
            (scope.opens_at_seconds() - 2.0).abs() < 1e-9,
            "four beats at 120 bpm, got {}",
            scope.opens_at_seconds()
        );
    }

    #[test]
    fn the_guard_is_every_lookahead_in_the_path_added_up() {
        let compress = |attack| Fx::Compress {
            threshold: -20.0,
            ratio: 4.0,
            attack,
            release: 0.1,
            makeup: 0.0,
            mix: 1.0,
            sidechain: None,
        };
        let mut song = two_tracks();
        assert_eq!(
            guard(&song),
            ((fx::limiter::LOOKAHEAD + SEAM) * RATE).ceil() as usize + RECONSTRUCTION,
            "a song with no chains still waits for the limiter and the seam"
        );
        song.tracks[0].fx = vec![compress(0.05)];
        song.tracks[1].fx = vec![compress(0.01)];
        song.fx = vec![compress(0.03)];
        assert_eq!(
            guard(&song),
            ((0.05 + 0.03 + fx::limiter::LOOKAHEAD + SEAM) * RATE).ceil() as usize + RECONSTRUCTION,
            "the loudest-reaching track and the song chain add; the other track does not"
        );
    }

    #[test]
    fn an_excerpt_says_what_was_asked_for() {
        let window = Window::beats("0:32".parse::<Span>().expect("a legal span"));
        assert_eq!(Excerpt::of(window).to_string(), "beats 0:32");
        let open = Window::seconds("12:".parse::<Span>().expect("a legal span"));
        assert_eq!(
            Excerpt {
                window: Some(open),
                only: vec!["pad".to_owned(), "sub".to_owned()],
            }
            .to_string(),
            "seconds 12:, only pad + sub"
        );
        assert_eq!(Excerpt::default().to_string(), "all of it");
    }

    #[test]
    fn beats_become_samples_at_the_rendered_tempo() {
        let window = Window::beats(Span::new(4.0, Some(8.0)).unwrap());
        let (from, to) = window.frames(&Clock::at(120.0));
        assert_eq!(
            from,
            RATE as usize * 2,
            "four beats at 120 bpm is two seconds"
        );
        assert_eq!(to, Some(RATE as usize * 4));
    }

    #[test]
    fn seconds_are_seconds_whatever_the_tempo() {
        let window = Window::seconds(Span::new(0.0, Some(12.0)).unwrap());
        assert_eq!(window.frames(&Clock::at(75.0)).1, Some(RATE as usize * 12));
    }

    #[test]
    fn an_open_window_renders_every_note() {
        let window = Window::beats(Span::new(4.0, None).unwrap());
        assert_eq!(window.frames(&Clock::at(120.0)).1, None);
    }
}
