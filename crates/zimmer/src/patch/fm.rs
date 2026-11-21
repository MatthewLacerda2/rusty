//! Four-operator FM: the operators, and the fixed table of ways to wire them.
//!
//! [`Source::Fm2`](super::Source::Fm2) is one sine bending another, which is
//! one modulator-carrier relationship and therefore one *kind* of sound —
//! bells, electric pianos, struck metal. Four operators reach the rest of FM
//! because the wiring becomes a choice: a modulator into a modulator into a
//! carrier is a very different sound from two modulators summed into one
//! carrier, which is different again from two carrier pairs mixed. That choice
//! is what a DX7 calls an **algorithm**, and it is the parameter this module
//! exists for.
//!
//! ## Why the choices are a closed list
//!
//! [The patch doc](super) is explicit that a patch is *structured, not a free
//! graph*, because `(pitch, velocity, duration) in → buffer out` has to always
//! hold. An operator routing written as an arbitrary edge list would break
//! exactly that: a recipe could describe a cycle with no output, or a stage
//! that never settles.
//!
//! So the routings are a **table**, and a recipe picks a row of it. Every row
//! is a DAG by construction, and the construction is stronger than a check —
//! **every edge in the table runs from a lower-numbered operator to a
//! higher-numbered one**, so evaluating operators in order 1, 2, 3, 4 is
//! always enough and a cycle cannot be written down. The one loop that does
//! exist is [`Operator::feedback`], which is an operator into itself through a
//! one-sample delay, bounded in radians by the renderer.
//!
//! ## Where the table comes from
//!
//! Transcribed from Yamaha's **four-operator FM** algorithm set — the eight
//! routings of the YM2151 (OPM) and YM2612 (OPN2), the chips behind a decade
//! of arcade and Mega Drive music, and the same eight the DX21/TX81Z family
//! puts on its front panel. It is published, reimplemented many times over,
//! and ordered here as those chips order it, so a patch written against a
//! hardware diagram transcribes across.
//!
//! Two of the eight — [`Algorithm::Branch`] and [`Algorithm::Fork`] — are the
//! same shape with the operators numbered differently, and that is faithful to
//! the source rather than an oversight. On the hardware, feedback is wired to
//! operator 1 and cannot be moved, so *which* operator sits at the head of the
//! two-deep chain is a real difference. Here feedback is a field on each
//! operator, so the two rows differ only in where a recipe writes its numbers
//! — kept because a row that means what the diagram means is worth more than a
//! shorter list.

use serde::{Deserialize, Serialize};

use super::Adsr;

/// How many operators a four-operator patch has. Four, and it is in the name:
/// the routing table is written for exactly this many, and a recipe writes
/// exactly this many.
pub const FM_OPERATORS: usize = 4;

/// One operator: a sine, its place relative to the played pitch, how loudly it
/// speaks, and how that changes over the note.
///
/// **What `level` means depends on what the algorithm made this operator.** A
/// carrier's level is its weight in the mix — the carriers are normalised by
/// their total, so raising one lifts its share rather than the volume. A
/// modulator's level is its **index**: modulation depth in radians, the same
/// quantity and the same unit as [`Source::Fm2`](super::Source::Fm2)'s
/// `index`, so `1` is a gentle colouring, `5` is bright and `10` is
/// aggressive. No operator is ever both under any row of the table, so the
/// number is never ambiguous — [`Algorithm::is_carrier`] says which it is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    /// Frequency as a multiple of the played pitch. Whole numbers stay
    /// harmonic; fractional ones (`1.41`, `3.5`) go inharmonic, which is where
    /// metal and glass live.
    ///
    /// Refused if it is not positive, for the reason
    /// [`Source::Fm2`](super::Source::Fm2)'s is: a multiple of the pitch that
    /// is zero or negative describes no sound.
    pub ratio: f32,
    /// Its weight in the mix if the algorithm makes it a carrier, or its
    /// modulation index in radians if it makes it a modulator — see the type
    /// doc. Defaults to `1.0`.
    #[serde(default = "one")]
    pub level: f32,
    /// How much of its own output bends its own phase, `0..=1`.
    ///
    /// Self-modulation is the standard way an FM voice reaches the rasping,
    /// noise-like end of its range: low down it fattens a sine toward a saw,
    /// and near the top the operator breaks up into a growl. Defaults to
    /// `0.0`, which is no loop at all.
    ///
    /// **It cannot run away.** The renderer scales the fed-back sample to a
    /// bounded number of radians and adds it to a phase, and a phase offset
    /// only moves where a sine is read — so the operator's output is a sine's
    /// output whatever this says, and the loop is one sample deep rather than
    /// a recursion.
    #[serde(default)]
    pub feedback: f32,
    /// Its own envelope over the note, or absent for an operator held at full
    /// level throughout.
    ///
    /// **This is most of what makes four operators expressive**, and it is the
    /// half two operators cannot do: a modulator on a fast decay over a
    /// carrier that sustains *is* a bright attack settling into a body, and
    /// the two are separable only because each operator has its own shape. The
    /// same envelope type the amp stage uses, driven by the same gate.
    ///
    /// The amp envelope multiplies whatever this leaves, so **the shorter of
    /// the two always wins**: an operator whose envelope has closed is gone
    /// for the rest of the note however the amp envelope moves afterwards, and
    /// an operator that would ring for ten seconds under an amp envelope that
    /// releases in 50 ms is a 50 ms note. A carrier written with a short
    /// release under a long amp release cuts its own tail off, which is worth
    /// knowing before it is heard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Adsr>,
}

fn one() -> f32 {
    1.0
}

/// How the four operators are wired: which of them modulate which, and which
/// of them are heard.
///
/// The eight routings of Yamaha's four-operator chips — the YM2151 (OPM) and
/// YM2612 (OPN2), and the same eight the DX21/TX81Z family puts on its panel —
/// transcribed in the order those chips list them, so a patch written against
/// a hardware diagram carries across.
///
/// **The list is closed, and that is the design.** A routing a recipe could
/// draw for itself would let it describe a cycle with no output or a stage
/// that never settles, which is the one thing a [`Patch`](super::Patch) may
/// never be. Every row here is a DAG by construction: each edge runs from a
/// lower-numbered operator to a higher-numbered one, so a single pass in
/// operator order is always enough and a cycle cannot be written down.
///
/// Operators are numbered **1 to 4** in the diagrams below, matching the order
/// a recipe writes them in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    /// `1 → 2 → 3 → 4`. One carrier at the end of a three-deep stack: the
    /// brightest and most extreme routing, and the one that goes inharmonic
    /// fastest.
    Chain,
    /// `(1 + 2) → 3 → 4`. Two modulators summed into the top of a chain — two
    /// unrelated ratios colouring one wave, and that wave shaping the carrier.
    Stack,
    /// `2 → 3`, then `(1 + 3) → 4`. The carrier is bent by one plain sine and
    /// by one that has itself been bent: a simple partial and a complex one on
    /// the same tone.
    Branch,
    /// `1 → 2`, then `(2 + 3) → 4`. [`Algorithm::Branch`]'s shape with the
    /// operators renumbered, and kept because the diagrams keep it: on the
    /// hardware, feedback is wired to operator 1 and cannot be moved, so which
    /// operator heads the two-deep chain is a real difference there. Here
    /// [`Operator::feedback`] is per operator, so the two rows differ only in
    /// where a recipe writes its numbers.
    Fork,
    /// `(1 → 2) + (3 → 4)`. Two independent two-operator voices, mixed. The
    /// workhorse: a bright, fast pair layered over a slow, warm one is how a
    /// horn section, an electric piano and most FM basses are built.
    Twin,
    /// `1 → (2, 3, 4)`. One modulator into three carriers at once — a chorus
    /// of related partials moving together. Organ-ish and choral.
    Fan,
    /// `(1 → 2) + 3 + 4`. One modulated voice beside two plain sines: an FM
    /// tone with an additive body underneath it.
    PairAndTwo,
    /// `1 + 2 + 3 + 4`. Four sines, no modulation at all — the additive corner
    /// of the table, and the flat floor a patch can start from.
    Parallel,
}

/// One row of the table, as bitmasks over the four operators.
///
/// Bitmasks rather than lists of edges because they are read once per operator
/// per sample, and because they make the invariant the module doc claims a
/// thing a test can state: `inputs[i]` may only carry bits below `i`.
#[derive(Clone, Copy)]
struct Routing {
    /// `inputs[i]` — which operators modulate operator `i`.
    inputs: [u8; FM_OPERATORS],
    /// Which operators are heard directly.
    carriers: u8,
}

/// The eight routings, in [`Algorithm`]'s order, so a variant indexes it.
const TABLE: [Routing; 8] = [
    // Chain: 1 → 2 → 3 → 4.
    Routing {
        inputs: [0b0000, 0b0001, 0b0010, 0b0100],
        carriers: 0b1000,
    },
    // Stack: (1 + 2) → 3 → 4.
    Routing {
        inputs: [0b0000, 0b0000, 0b0011, 0b0100],
        carriers: 0b1000,
    },
    // Branch: 2 → 3, (1 + 3) → 4.
    Routing {
        inputs: [0b0000, 0b0000, 0b0010, 0b0101],
        carriers: 0b1000,
    },
    // Fork: 1 → 2, (2 + 3) → 4.
    Routing {
        inputs: [0b0000, 0b0001, 0b0000, 0b0110],
        carriers: 0b1000,
    },
    // Twin: (1 → 2) + (3 → 4).
    Routing {
        inputs: [0b0000, 0b0001, 0b0000, 0b0100],
        carriers: 0b1010,
    },
    // Fan: 1 → (2, 3, 4).
    Routing {
        inputs: [0b0000, 0b0001, 0b0001, 0b0001],
        carriers: 0b1110,
    },
    // PairAndTwo: (1 → 2) + 3 + 4.
    Routing {
        inputs: [0b0000, 0b0001, 0b0000, 0b0000],
        carriers: 0b1110,
    },
    // Parallel: 1 + 2 + 3 + 4.
    Routing {
        inputs: [0b0000, 0b0000, 0b0000, 0b0000],
        carriers: 0b1111,
    },
];

impl Algorithm {
    /// Every algorithm there is, so anything that has to cover the table
    /// enumerates it from one place rather than restating the list.
    ///
    /// Published because the list is a *document* fact as much as a code one:
    /// `docs/recipes.md` carries a row per routing, and a table that could
    /// silently fall behind the code is the failure the documentation gate
    /// exists for. The test that holds the page to this is what makes adding a
    /// ninth algorithm also add its row.
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

    /// What this routing is called, in the word the document spells it with.
    ///
    /// The serde tag rather than a prose label, for the reason
    /// [`Source::kind`](super::Source::kind) is: an error that names an
    /// algorithm and a recipe that chooses one have to use the same
    /// vocabulary, or the reader has to translate between them.
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

    /// This algorithm's row of the table.
    fn routing(self) -> &'static Routing {
        &TABLE[self as usize]
    }

    /// Whether operator `index` — counting from **zero**, so operator 1 of the
    /// diagrams is `0` — reaches the output directly.
    ///
    /// The distinction the whole document rests on: a carrier is heard and its
    /// [`Operator::level`] is a mix weight, a modulator is not heard and its
    /// level is an index in radians. Under every row of the table an operator
    /// is one or the other and never both.
    pub fn is_carrier(self, index: usize) -> bool {
        self.routing().carriers & bit(index) != 0
    }

    /// Which operators modulate operator `index`, as a bitmask.
    ///
    /// Only bits below `index` are ever set — the invariant that makes
    /// evaluating the operators in order both correct and terminating.
    pub(crate) fn modulators(self, index: usize) -> u8 {
        match self.routing().inputs.get(index) {
            Some(mask) => *mask,
            None => 0,
        }
    }
}

/// Operator `index` as a one-bit mask, and nothing at all for an index past
/// the four that exist.
fn bit(index: usize) -> u8 {
    if index < FM_OPERATORS { 1 << index } else { 0 }
}

/// The table by its own claimed invariants, which is what makes "every routing
/// terminates" a fact rather than an intention.
#[cfg(test)]
mod tests {
    use super::*;

    /// Every operator that modulates anything, across a whole algorithm.
    fn modulating(algorithm: Algorithm) -> u8 {
        (0..FM_OPERATORS).fold(0, |all, i| all | algorithm.modulators(i))
    }

    /// Every edge runs from a lower operator to a higher one. This is the
    /// whole termination argument: a strictly increasing edge order cannot
    /// contain a cycle, so evaluating 1, 2, 3, 4 in order always has its
    /// inputs already in hand.
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

    /// No operator is both heard and used as a modulator, which is what lets
    /// [`Operator::level`] mean one thing per operator rather than two.
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

    /// Every routing is heard, and every operator in it does something — a row
    /// with an idle operator would be a knob a recipe could turn for no reason.
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

    /// The rows the diagrams claim, spelled out as carriers — the half of the
    /// table nothing else can get wrong quietly, since it decides what is
    /// heard at all.
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

    /// And the modulator edges, one row at a time, written as the diagram
    /// reads rather than as the mask is stored — so a transcription slip shows
    /// up here as the wrong operator rather than as a number.
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

    /// The name is the word the document is written with, which is only true
    /// if serde spells it the same way. Two places state it and this is what
    /// keeps them from drifting apart.
    #[test]
    fn every_name_is_the_word_serde_writes() {
        for algorithm in Algorithm::ALL {
            let json = serde_json::to_string(&algorithm).expect("an algorithm serialises");
            assert_eq!(json, format!("\"{}\"", algorithm.name()));
        }
    }

    /// An index past the four operators is nothing rather than a panic:
    /// `is_carrier` is public, and a caller's number is not this module's to
    /// trust.
    #[test]
    fn an_index_past_the_four_operators_is_nothing() {
        assert!(!Algorithm::Parallel.is_carrier(FM_OPERATORS));
        assert_eq!(Algorithm::Chain.modulators(FM_OPERATORS), 0);
        assert_eq!(bit(FM_OPERATORS), 0);
    }
}
