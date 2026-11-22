use glam::Vec2;

use super::{Fault, Parser};

impl Parser<'_> {
    pub(super) fn pair(&mut self) -> Result<Vec2, Fault> {
        let x = self.number()?;
        let y = self.number()?;
        Ok(Vec2::new(x, y))
    }

    pub(super) fn flag(&mut self) -> Result<bool, Fault> {
        self.separator();
        let flag = match self.src.get(self.at) {
            Some(b'0') => false,
            Some(b'1') => true,
            _ => return Err(self.fault("expected an arc flag, `0` or `1`")),
        };
        self.at += 1;
        Ok(flag)
    }

    pub(super) fn number(&mut self) -> Result<f32, Fault> {
        self.separator();
        let begin = self.at;
        let digits = |p: &mut Self| {
            let from = p.at;
            while p.src.get(p.at).is_some_and(u8::is_ascii_digit) {
                p.at += 1;
            }
            p.at > from
        };
        if matches!(self.src.get(self.at), Some(b'+' | b'-')) {
            self.at += 1;
        }
        let mut any = digits(self);
        if self.src.get(self.at) == Some(&b'.') {
            self.at += 1;
            any |= digits(self);
        }
        if !any {
            self.at = begin;
            return Err(self.fault("expected a number"));
        }
        if matches!(self.src.get(self.at), Some(b'e' | b'E')) {
            let mark = self.at;
            self.at += 1;
            if matches!(self.src.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !digits(self) {
                self.at = mark;
            }
        }
        let text = std::str::from_utf8(&self.src[begin..self.at]).unwrap_or("");
        match text.parse::<f32>() {
            Ok(v) if v.is_finite() => Ok(v),
            _ => {
                self.at = begin;
                Err(self.fault("expected a finite number"))
            }
        }
    }

    pub(super) fn separator(&mut self) {
        self.skip();
        if self.src.get(self.at) == Some(&b',') {
            self.at += 1;
            self.skip();
        }
    }

    pub(super) fn skip(&mut self) {
        while self.src.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    pub(super) fn fault(&self, what: &str) -> Fault {
        (self.at, what.to_string())
    }
}
