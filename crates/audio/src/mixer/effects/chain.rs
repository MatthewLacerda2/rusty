use serde::{Deserialize, Serialize};

use super::param::{self, Fields, Range, Value};
use super::{EffectChain, EffectParams, KindId, MAX_EFFECTS, MAX_PARAMS};
use crate::mixer::{GroupId, GroupPatch, Mixer};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectPatch {
    pub params: EffectParams,
    pub mask: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainPatch(pub [Option<EffectPatch>; MAX_EFFECTS]);

impl ChainPatch {
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

fn kind_of(fields: &Fields) -> Result<KindId, String> {
    match fields.iter().find(|(k, _)| k == "kind") {
        Some((_, Value::Text(name))) => KindId::named(name),
        _ => Err("an effect table needs kind = \"<name>\"".to_string()),
    }
}

impl Mixer {
    pub fn effects(&self, id: GroupId) -> &EffectChain {
        &self.groups[usize::from(id.0)].settings.effects
    }

    /// A script's effect tables as a chain: every parameter not named at its
    /// default. An unknown kind, field or group, or a value out of range, errs.
    pub fn parse_effects(&self, tables: &[Fields]) -> Result<EffectChain, String> {
        let resolve = |name: &str| match name {
            "" => Ok(None),
            name => self.require(name).map(Some),
        };
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

    pub fn parse_effect_patch(&self, id: GroupId, tables: &[Fields]) -> Result<ChainPatch, String> {
        let chain = self.effects(id);
        let mut patch = ChainPatch::default();
        for (slot, fields) in tables.iter().enumerate() {
            if fields.is_empty() {
                continue;
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

fn clear_key(effect: &mut EffectParams) {
    let row = effect.kind.row();
    if let Some(i) = row.params.iter().position(|p| p.range == Range::Group) {
        effect.values[i] = Range::Group.default();
    }
}
