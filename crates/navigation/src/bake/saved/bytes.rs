//! Little-endian words in and out.

pub(super) fn put(out: &mut Vec<u8>, v: u32) {
    out.extend(v.to_le_bytes());
}

pub(super) fn putf(out: &mut Vec<u8>, v: f32) {
    out.extend(v.to_le_bytes());
}

pub(super) struct Reader<'a>(pub &'a [u8]);

impl<'a> Reader<'a> {
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.0.len() < n {
            return Err("truncated".into());
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.take(N)?.try_into().expect("took N bytes"))
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn f32(&mut self) -> Result<f32, String> {
        self.array().map(f32::from_le_bytes)
    }
}
