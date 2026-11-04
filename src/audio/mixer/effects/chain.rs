//! src/audio/mixer/effects/chain.rs — the mixer's effect verbs: set a group's
//! chain from a script's tables, read it back, patch it from a snapshot, and keep
//! sidechains acyclic (#1748).
//!
//! **The order groups are processed in.** A group's signal holds its children's,
//! so children come before their parent (kira's mix tree), and a sidechain's key
//! group comes before the group it keys. [`Mixer::processing_order`] is that order.
//! A key the keyed group's own output reaches — the group itself, one of its
//! ancestors, or a loop of sidechains — has no such order: it is a **cycle**, and
//! [`Mixer::set_effects`] drops that compressor's sidechain with a warning (it then
//! compresses on its own input). On the device a compressor reads its key's peak
//! from the block before (128 frames, 2.7 ms at 48 kHz, `rack.rs`), so which of
//! two unrelated groups kira happens to run first never changes what it hears.

use serde::{Deserialize, Serialize};

use super::param::{self, Fields, Range, Value};
use super::{EffectChain, EffectParams, KindId, MAX_EFFECTS, MAX_PARAMS};
use crate::audio::mixer::{GroupId, GroupPatch, Mixer};

/// A snapshot's change to one effect: the values the bits of `mask` name, over the
/// group's effect at that slot when it is of the same kind.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectPatch {
    pub params: EffectParams,
    pub mask: u32,
}

/// A snapshot's changes to a group's effects, slot by slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainPatch(pub [Option<EffectPatch>; MAX_EFFECTS]);

impl ChainPatch {
    /// `chain` with every patched value written; a slot whose effect is now of
    /// another kind (the chain was set since) is left alone.
    pub fn apply(&self, chain: &EffectChain) -> EffectChain {
        let mut out = *chain;
        for (slot, patch) in out.0.iter_mut().zip(&self.0) {
            let (Some(e), Some(p)) = (slot.as_mut(), patch) else {
                continue;
            };
            if e.kind != p.params.kind {
                continue;
            }
            for i in (0..MAX_PARAMS).filter(|i| p.mask & (1 << i) != 0) {
                e.values[i] = p.params.values[i];
            }
        }
        out
    }
}

/// The `kind` field of a script's effect table.
fn kind_of(fields: &Fields) -> Result<KindId, String> {
    match fields.iter().find(|(k, _)| k == "kind") {
        Some((_, Value::Text(name))) => KindId::named(name),
        _ => Err("an effect table needs kind = \"<name>\"".to_string()),
    }
}

impl Mixer {
    /// Group `id`'s effects.
    pub fn effects(&self, id: GroupId) -> &EffectChain {
        &self.groups[usize::from(id.0)].settings.effects
    }

    /// A script's effect tables as a chain: every parameter not named at its
    /// default. An unknown kind, field or group, or a value out of range, errs.
    pub fn parse_effects(&self, tables: &[Fields]) -> Result<EffectChain, String> {
        let resolve = |name: &str| self.require(name);
        let effects = tables
            .iter()
            .map(|fields| {
                let kind = kind_of(fields)?;
                let base = EffectParams::defaults(kind);
                Ok(param::parse(kind, fields, &base, &resolve)?.0)
            })
            .collect::<Result<Vec<_>, String>>()?;
        EffectChain::of(&effects)
    }

    /// A snapshot's `effects` list for group `id`, slot by slot: each table names
    /// the kind the group has in that slot now, and the values it changes (`{}`
    /// leaves the slot alone). A sidechain cannot be changed by a snapshot.
    pub fn parse_effect_patch(&self, id: GroupId, tables: &[Fields]) -> Result<ChainPatch, String> {
        let chain = self.effects(id);
        let mut patch = ChainPatch::default();
        for (slot, fields) in tables.iter().enumerate() {
            if fields.is_empty() {
                continue; // `{}` leaves this slot alone
            }
            let kind = kind_of(fields)?;
            let Some(current) = chain.0.get(slot).copied().flatten() else {
                return Err(format!("the group has no effect {} to patch", slot + 1));
            };
            if current.kind != kind {
                let has = current.kind.row().name;
                return Err(format!(
                    "effect {} is a {has}, not a {}",
                    slot + 1,
                    kind.row().name
                ));
            }
            let refuse = |_: &str| Err("a snapshot cannot change a sidechain".to_string());
            let (params, mask) = param::parse(kind, fields, &current, &refuse)?;
            patch.0[slot] = Some(EffectPatch { params, mask });
        }
        Ok(patch)
    }

    /// Group `id`'s effects as a script reads them back.
    pub fn effect_fields(&self, id: GroupId) -> Vec<Fields> {
        let name = |g: GroupId| self.groups[usize::from(g.0)].name.clone();
        self.effects(id)
            .iter()
            .map(|e| param::emit(e, &name))
            .collect()
    }

    /// Replace group `id`'s effects with `chain`. A sidechain that would make a
    /// cycle (module docs) is dropped, with a warning naming it.
    pub fn set_effects(&mut self, id: GroupId, mut chain: EffectChain) {
        self.groups[usize::from(id.0)].settings.effects = EffectChain::default();
        let mut keys = Vec::new();
        for effect in chain.0.iter_mut().flatten() {
            let Some(key) = effect.key() else { continue };
            keys.push((key, id));
            if self.order_with(&keys).is_none() {
                keys.pop();
                log::warn!(
                    "[Audio] group '{}' keying a {} from '{}' is a cycle; no sidechain",
                    self.groups[usize::from(id.0)].name,
                    effect.kind.row().name,
                    self.groups[usize::from(key.0)].name,
                );
                clear_key(effect);
            }
        }
        let patch = GroupPatch {
            effects: Some(chain),
            ..GroupPatch::default()
        };
        self.set(id, &patch);
    }

    /// Every group in the order they are processed: children and sidechain keys
    /// before the groups that need them (module docs).
    pub fn processing_order(&self) -> Vec<GroupId> {
        self.order_with(&[]).unwrap_or_default()
    }

    /// The processing order with the `(key, keyed)` edges of `extra` added, or
    /// `None` when there is none (a cycle). Kahn's algorithm, lowest id first.
    fn order_with(&self, extra: &[(GroupId, GroupId)]) -> Option<Vec<GroupId>> {
        let n = self.groups.len();
        let mut edges: Vec<(usize, usize)> = extra
            .iter()
            .map(|(a, b)| (usize::from(a.0), usize::from(b.0)))
            .collect();
        for (i, g) in self.groups.iter().enumerate() {
            if let Some(p) = g.parent {
                edges.push((i, usize::from(p.0)));
            }
            for key in g.settings.effects.iter().filter_map(EffectParams::key) {
                edges.push((usize::from(key.0), i));
            }
        }
        let mut needs = vec![0usize; n];
        for &(_, b) in &edges {
            needs[b] += 1;
        }
        let mut order = Vec::with_capacity(n);
        let mut done = vec![false; n];
        while let Some(next) = (0..n).find(|&i| !done[i] && needs[i] == 0) {
            done[next] = true;
            order.push(GroupId(next as u16));
            for &(_, b) in edges.iter().filter(|(a, _)| *a == next) {
                needs[b] -= 1;
            }
        }
        (order.len() == n).then_some(order)
    }
}

/// Unset `effect`'s sidechain parameter.
fn clear_key(effect: &mut EffectParams) {
    let row = effect.kind.row();
    if let Some(i) = row.params.iter().position(|p| p.range == Range::Group) {
        effect.values[i] = Range::Group.default();
    }
}
