use std::collections::BTreeMap;

use super::Decal;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Counts {
    whole: BTreeMap<Option<u32>, u32>,
    pub total: u32,
    pub evicting: usize,
}

impl Counts {
    pub fn whole(&self, bucket: Option<u32>) -> u32 {
        self.whole.get(&bucket).copied().unwrap_or(0)
    }

    pub fn add(&mut self, bucket: Option<u32>) {
        *self.whole.entry(bucket).or_default() += 1;
        self.total += 1;
    }

    pub fn sub(&mut self, bucket: Option<u32>) {
        if let Some(n) = self.whole.get_mut(&bucket) {
            *n -= 1;
            if *n == 0 {
                self.whole.remove(&bucket);
            }
            self.total -= 1;
        }
    }

    pub fn forget(&mut self, decal: &Decal) {
        if !decal.retiring {
            self.sub(decal.bucket());
        } else if decal.evicted {
            self.evicting -= 1;
        }
    }
}
