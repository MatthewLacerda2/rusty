//! `d` comes from disk, so nothing here may panic, recurse or grow without bound:
//! `d` is at most [`MAX_BYTES`] long, the flattened points at most
//! [`MAX_POINTS`], and a point past ±[`REACH`] tiles is refused. A parse refusal
//! names the byte where it happened.

mod curve;
mod lex;

use glam::Vec2;

pub const MAX_BYTES: usize = 1 << 16;
pub const MAX_POINTS: usize = 1 << 18;
pub const REACH: f32 = 4.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Subpath {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

pub fn parse(d: &str) -> Result<Vec<Subpath>, String> {
    if d.len() > MAX_BYTES {
        return Err(format!(
            "is {} bytes, past the {MAX_BYTES} a path takes",
            d.len()
        ));
    }
    let mut p = Parser {
        src: d.as_bytes(),
        at: 0,
        out: Vec::new(),
        points: 0,
        pen: Vec2::ZERO,
        start: Vec2::ZERO,
        last_control: None,
    };
    p.run()
        .map_err(|(at, what)| format!("at byte {at}: {what}"))?;
    // A lone point is dropped unless `Z` closed it: stroked, that is a dot.
    Ok(p.out
        .into_iter()
        .filter(|s| s.points.len() > 1 || s.closed)
        .collect())
}

type Fault = (usize, String);

struct Parser<'a> {
    src: &'a [u8],
    at: usize,
    out: Vec<Subpath>,
    points: usize,
    pen: Vec2,
    start: Vec2,
    last_control: Option<(Vec2, bool)>,
}

impl Parser<'_> {
    fn run(&mut self) -> Result<(), Fault> {
        self.skip();
        let mut command = None;
        while self.at < self.src.len() {
            let c = self.src[self.at];
            if self.out.is_empty() && !matches!(c, b'M' | b'm') {
                return Err(self.fault("a path starts with `M` or `m`"));
            }
            if c.is_ascii_alphabetic() {
                self.at += 1;
                command = Some(c);
            }
            let Some(cmd) = command else {
                return Err(self.fault("expected a command letter"));
            };
            self.command(cmd)?;
            // Coordinates after a move are lines; `Z` takes none.
            command = match cmd {
                b'M' => Some(b'L'),
                b'm' => Some(b'l'),
                b'Z' | b'z' => None,
                c => Some(c),
            };
            self.skip();
        }
        Ok(())
    }

    fn command(&mut self, cmd: u8) -> Result<(), Fault> {
        let rel = cmd.is_ascii_lowercase();
        let origin = if rel { self.pen } else { Vec2::ZERO };
        let mut control = None;
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let to = origin + self.pair()?;
                self.out.push(Subpath {
                    points: Vec::new(),
                    closed: false,
                });
                self.start = to;
                self.line(to)?;
            }
            b'L' => {
                let to = origin + self.pair()?;
                self.line(to)?;
            }
            b'H' => {
                let x = self.number()? + origin.x;
                self.line(Vec2::new(x, self.pen.y))?;
            }
            b'V' => {
                let y = self.number()? + origin.y;
                self.line(Vec2::new(self.pen.x, y))?;
            }
            kind @ (b'C' | b'S' | b'Q' | b'T' | b'A') => control = self.curve(kind, origin)?,
            b'Z' => self.close(),
            _ => {
                self.at -= 1;
                return Err(self.fault(&format!("unknown command `{}`", cmd as char)));
            }
        }
        self.last_control = control;
        Ok(())
    }

    fn close(&mut self) {
        if let Some(s) = self.out.last_mut() {
            s.closed = true;
        }
        let start = self.start;
        self.pen = start;
        self.out.push(Subpath {
            points: vec![start],
            closed: false,
        });
        self.points += 1;
    }

    fn reflected(&self, cubic: bool) -> Vec2 {
        match self.last_control {
            Some((c, was_cubic)) if was_cubic == cubic => 2.0 * self.pen - c,
            _ => self.pen,
        }
    }

    fn line(&mut self, to: Vec2) -> Result<(), Fault> {
        if !(to.x.abs() <= REACH && to.y.abs() <= REACH) {
            return Err(self.fault(&format!(
                "the point ({}, {}) lies past ±{REACH} tiles",
                to.x, to.y
            )));
        }
        if self.points >= MAX_POINTS {
            return Err(self.fault(&format!("the path flattens to past {MAX_POINTS} points")));
        }
        self.points += 1;
        match self.out.last_mut() {
            Some(s) => s.points.push(to),
            None => unreachable!("a path starts with a move"),
        }
        self.pen = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
