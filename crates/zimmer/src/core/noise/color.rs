//! the colour of noise: how a draw's energy is spread across the spectrum.

use crate::patch::NoiseColor;

pub(crate) enum Coloring {
    White,
    Pink { poles: [f32; PINK_POLES.len()] },
    Brown { level: f32 },
}

const PINK_POLES: [(f32, f32); 3] = [
    (0.99765, 0.099_046_0),
    (0.96300, 0.296_516_4),
    (0.57000, 1.052_691_3),
];

const PINK_DIRECT: f32 = 0.1848;

const PINK_SCALE: f32 = 0.3372;

const BROWN_LEAK: f32 = 0.995;

const BROWN_SCALE: f32 = 0.1009;

pub(crate) const WARMUP: usize = 2048;

impl Coloring {
    pub(crate) fn new(color: NoiseColor) -> Self {
        match color {
            NoiseColor::White => Self::White,
            NoiseColor::Pink => Self::Pink {
                poles: [0.0; PINK_POLES.len()],
            },
            NoiseColor::Brown => Self::Brown { level: 0.0 },
        }
    }

    pub(crate) fn warmup(&self) -> usize {
        match self {
            Self::White => 0,
            Self::Pink { .. } | Self::Brown { .. } => WARMUP,
        }
    }

    pub(crate) fn step(&mut self, white: f32) -> f32 {
        match self {
            Self::White => white,
            Self::Pink { poles } => {
                let mut sum = white * PINK_DIRECT;
                for (pole, (decay, feed)) in poles.iter_mut().zip(PINK_POLES) {
                    *pole = decay * *pole + white * feed;
                    sum += *pole;
                }
                sum * PINK_SCALE
            }
            Self::Brown { level } => {
                *level = BROWN_LEAK * *level + white;
                *level * BROWN_SCALE
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brown_integrator_leaks_rather_than_running_away() {
        const { assert!(BROWN_LEAK < 1.0, "an unleaky integrator is a random walk") };
        let mut brown = Coloring::new(NoiseColor::Brown);
        let mut last = 0.0;
        for _ in 0..200_000 {
            last = brown.step(1.0);
        }
        let settled = BROWN_SCALE / (1.0 - BROWN_LEAK);
        assert!(
            (last - settled).abs() < 0.01 * settled,
            "a held input settles at {settled}, not {last}"
        );
    }

    #[test]
    fn the_brown_integrator_forgets_what_it_was_handed() {
        let mut brown = Coloring::new(NoiseColor::Brown);
        let struck = brown.step(1.0);
        let mut level = struck;
        for _ in 0..1000 {
            level = brown.step(0.0);
        }
        assert!(level.abs() < 0.01 * struck, "still holding {level}");
    }

    #[test]
    fn white_is_the_draw_itself() {
        let mut white = Coloring::new(NoiseColor::White);
        assert_eq!(white.warmup(), 0);
        for sample in [-1.0, -0.25, 0.0, 0.3, 1.0] {
            assert_eq!(white.step(sample), sample);
        }
    }

    #[test]
    fn every_coloured_filter_asks_to_be_warmed() {
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            assert_eq!(Coloring::new(color).warmup(), WARMUP, "{color:?}");
        }
        const { assert!(WARMUP > 4 * 425, "the slowest pink pole needs longer") };
    }

    #[test]
    fn all_three_pink_poles_reach_the_output() {
        let draws: Vec<f32> = (0..4096)
            .map(|i| ((i * 37 % 101) as f32 / 50.0) - 1.0)
            .collect();
        let run = |skip: Option<usize>| {
            let mut poles = [0.0f32; PINK_POLES.len()];
            let mut out = Vec::with_capacity(draws.len());
            for white in &draws {
                let mut sum = white * PINK_DIRECT;
                for (at, (pole, (decay, feed))) in poles.iter_mut().zip(PINK_POLES).enumerate() {
                    *pole = decay * *pole + white * feed;
                    if Some(at) != skip {
                        sum += *pole;
                    }
                }
                out.push(sum * PINK_SCALE);
            }
            out
        };
        let whole = run(None);
        let mut filter = Coloring::new(NoiseColor::Pink);
        let through: Vec<f32> = draws.iter().map(|w| filter.step(*w)).collect();
        assert_eq!(whole, through, "the bank is not the sum of its poles");
        for at in 0..PINK_POLES.len() {
            assert_ne!(run(Some(at)), whole, "pole {at} changes nothing");
        }
        const { assert!(PINK_DIRECT > 0.0, "the raw draw is part of the sum") };
    }
}
