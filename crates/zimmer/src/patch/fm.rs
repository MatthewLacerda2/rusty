//! Four-operator FM: the operators, and the fixed table of ways to wire them.

use serde::{Deserialize, Serialize};

use super::Adsr;

pub const FM_OPERATORS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    pub ratio: f32,
    #[serde(default = "one")]
    pub level: f32,
    #[serde(default)]
    pub feedback: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Adsr>,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    Chain,
    Stack,
    Branch,
    Fork,
    Twin,
    Fan,
    PairAndTwo,
    Parallel,
}

#[derive(Clone, Copy)]
struct Routing {
    inputs: [u8; FM_OPERATORS],
    carriers: u8,
}

const TABLE: [Routing; 8] = [
    Routing {
        inputs: [0b0000, 0b0001, 0b0010, 0b0100],
        carriers: 0b1000,
    },
    Routing {
        inputs: [0b0000, 0b0000, 0b0011, 0b0100],
        carriers: 0b1000,
    },
    Routing {
        inputs: [0b0000, 0b0000, 0b0010, 0b0101],
        carriers: 0b1000,
    },
    Routing {
        inputs: [0b0000, 0b0001, 0b0000, 0b0110],
        carriers: 0b1000,
    },
    Routing {
        inputs: [0b0000, 0b0001, 0b0000, 0b0100],
        carriers: 0b1010,
    },
    Routing {
        inputs: [0b0000, 0b0001, 0b0001, 0b0001],
        carriers: 0b1110,
    },
    Routing {
        inputs: [0b0000, 0b0001, 0b0000, 0b0000],
        carriers: 0b1110,
    },
    Routing {
        inputs: [0b0000, 0b0000, 0b0000, 0b0000],
        carriers: 0b1111,
    },
];

impl Algorithm {
    pub const ALL: [Self; 8] = [
        Self::Chain,
        Self::Stack,
        Self::Branch,
        Self::Fork,
        Self::Twin,
        Self::Fan,
        Self::PairAndTwo,
        Self::Parallel,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Chain => "chain",
            Self::Stack => "stack",
            Self::Branch => "branch",
            Self::Fork => "fork",
            Self::Twin => "twin",
            Self::Fan => "fan",
            Self::PairAndTwo => "pair_and_two",
            Self::Parallel => "parallel",
        }
    }

    fn routing(self) -> &'static Routing {
        &TABLE[self as usize]
    }

    pub fn is_carrier(self, index: usize) -> bool {
        self.routing().carriers & bit(index) != 0
    }

    pub(crate) fn modulators(self, index: usize) -> u8 {
        match self.routing().inputs.get(index) {
            Some(mask) => *mask,
            None => 0,
        }
    }
}

fn bit(index: usize) -> u8 {
    if index < FM_OPERATORS { 1 << index } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modulating(algorithm: Algorithm) -> u8 {
        (0..FM_OPERATORS).fold(0, |all, i| all | algorithm.modulators(i))
    }

    #[test]
    fn every_edge_runs_from_a_lower_operator_to_a_higher_one() {
        for algorithm in Algorithm::ALL {
            for index in 0..FM_OPERATORS {
                let mask = algorithm.modulators(index);
                assert_eq!(
                    mask >> index,
                    0,
                    "{algorithm:?}: operator {index} is fed by {mask:#06b}, \
                     which reaches itself or above it"
                );
            }
        }
    }

    #[test]
    fn no_operator_is_both_a_carrier_and_a_modulator() {
        for algorithm in Algorithm::ALL {
            for index in 0..FM_OPERATORS {
                assert!(
                    !(algorithm.is_carrier(index) && modulating(algorithm) & bit(index) != 0),
                    "{algorithm:?}: operator {index} is both"
                );
            }
        }
    }

    #[test]
    fn every_routing_has_a_carrier_and_no_idle_operator() {
        for algorithm in Algorithm::ALL {
            assert_ne!(algorithm.routing().carriers, 0, "{algorithm:?} is silent");
            for index in 0..FM_OPERATORS {
                assert!(
                    algorithm.is_carrier(index) || modulating(algorithm) & bit(index) != 0,
                    "{algorithm:?}: operator {index} reaches nothing"
                );
            }
        }
    }

    #[test]
    fn the_carriers_are_the_ones_the_diagrams_name() {
        let carriers = |algorithm: Algorithm| {
            (0..FM_OPERATORS)
                .filter(|i| algorithm.is_carrier(*i))
                .map(|i| i + 1)
                .collect::<Vec<_>>()
        };
        assert_eq!(carriers(Algorithm::Chain), vec![4]);
        assert_eq!(carriers(Algorithm::Stack), vec![4]);
        assert_eq!(carriers(Algorithm::Branch), vec![4]);
        assert_eq!(carriers(Algorithm::Fork), vec![4]);
        assert_eq!(carriers(Algorithm::Twin), vec![2, 4]);
        assert_eq!(carriers(Algorithm::Fan), vec![2, 3, 4]);
        assert_eq!(carriers(Algorithm::PairAndTwo), vec![2, 3, 4]);
        assert_eq!(carriers(Algorithm::Parallel), vec![1, 2, 3, 4]);
    }

    #[test]
    fn the_modulator_edges_are_the_ones_the_diagrams_draw() {
        let edges = |algorithm: Algorithm| {
            let mut found = Vec::new();
            for target in 0..FM_OPERATORS {
                for source in 0..FM_OPERATORS {
                    if algorithm.modulators(target) & bit(source) != 0 {
                        found.push((source + 1, target + 1));
                    }
                }
            }
            found
        };
        assert_eq!(edges(Algorithm::Chain), vec![(1, 2), (2, 3), (3, 4)]);
        assert_eq!(edges(Algorithm::Stack), vec![(1, 3), (2, 3), (3, 4)]);
        assert_eq!(edges(Algorithm::Branch), vec![(2, 3), (1, 4), (3, 4)]);
        assert_eq!(edges(Algorithm::Fork), vec![(1, 2), (2, 4), (3, 4)]);
        assert_eq!(edges(Algorithm::Twin), vec![(1, 2), (3, 4)]);
        assert_eq!(edges(Algorithm::Fan), vec![(1, 2), (1, 3), (1, 4)]);
        assert_eq!(edges(Algorithm::PairAndTwo), vec![(1, 2)]);
        assert_eq!(edges(Algorithm::Parallel), Vec::new());
    }

    #[test]
    fn every_name_is_the_word_serde_writes() {
        for algorithm in Algorithm::ALL {
            let json = serde_json::to_string(&algorithm).expect("an algorithm serialises");
            assert_eq!(json, format!("\"{}\"", algorithm.name()));
        }
    }

    #[test]
    fn an_index_past_the_four_operators_is_nothing() {
        assert!(!Algorithm::Parallel.is_carrier(FM_OPERATORS));
        assert_eq!(Algorithm::Chain.modulators(FM_OPERATORS), 0);
        assert_eq!(bit(FM_OPERATORS), 0);
    }
}
