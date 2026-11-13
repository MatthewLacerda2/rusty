//! The running decal counts behind the budgets (#1517): kept in step with every
//! add, fade and drop, so a spawn checks its budgets without a pass over the ring.

use std::collections::BTreeMap;

use super::Decal;

/// What [`super::DecalSet`] holds the budgets against.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Counts {
    /// Whole decals per bucket ([`Decal::bucket`]); a bucket with none has no entry,
    /// so a destroyed owner leaves nothing behind.
    whole: BTreeMap<Option<u32>, u32>,
    /// Whole decals in the scene: the sum of `whole`.
    pub total: u32,
    /// Decals fading because a spawn evicted them ([`Decal::evicted`]).
    pub evicting: usize,
}

impl Counts {
    /// Whole decals in `bucket`.
    pub fn whole(&self, bucket: Option<u32>) -> u32 {
        self.whole.get(&bucket).copied().unwrap_or(0)
    }

    /// A whole decal joins `bucket`.
    pub fn add(&mut self, bucket: Option<u32>) {
        *self.whole.entry(bucket).or_default() += 1;
        self.total += 1;
    }

    /// A whole decal of `bucket` starts fading, or leaves.
    pub fn sub(&mut self, bucket: Option<u32>) {
        if let Some(n) = self.whole.get_mut(&bucket) {
            *n -= 1;
            if *n == 0 {
                self.whole.remove(&bucket);
            }
            self.total -= 1;
        }
    }

    /// `decal` leaves the ring, whole or fading.
    pub fn forget(&mut self, decal: &Decal) {
        if !decal.retiring {
            self.sub(decal.bucket());
        } else if decal.evicted {
            self.evicting -= 1;
        }
    }
}
